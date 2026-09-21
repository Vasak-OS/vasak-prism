//! Los archivos, leídos del índice que ya mantiene el gestor de archivos.
//!
//! # Por qué no hay un índice propio
//!
//! `vasak-file-manager` ya recorre el disco y mantiene un índice de tantivy con
//! los nombres de los archivos. Armar otro acá sería recorrer el mismo disco dos
//! veces, guardar los mismos nombres dos veces y vigilarlos dos veces — y que
//! los dos se contradigan cuando uno de los dos se arregle.
//!
//! Tantivy admite **muchos lectores y un escritor**, y los lectores no necesitan
//! que el escritor esté vivo. Así que acá se abre el índice de sólo lectura. El
//! gestor sigue siendo su dueño: lo crea, lo llena y lo actualiza; el lanzador
//! sólo pregunta.
//!
//! Lo que eso cuesta, dicho de frente: el lanzador queda atado a **dónde** está
//! el índice y a **qué forma** tiene. Si el gestor cambia el esquema, acá dejan
//! de aparecer archivos. Por eso nada de esto falla ruidosamente —sin índice o
//! con un índice que no se entiende, no hay filas y el resto del lanzador anda
//! igual— y por eso los nombres de los campos están en un solo lugar.
//!
//! Lo que **no** resuelve: la frescura. El índice es tan nuevo como el último
//! escaneo que alguien haya pedido desde el gestor. Cambiar eso es cambiar de
//! dueño, y es una decisión aparte.

use std::path::{Path, PathBuf};

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query};
use tantivy::schema::Value;
use tantivy::{Index, IndexReader, TantivyDocument, Term};

use crate::catalogo::aplicacion::{Origen, Resultado};
use crate::catalogo::puntaje;

/// Dónde está el índice, en orden de preferencia.
///
/// # Por qué son varias y no una
///
/// El índice cambia de dueño: lo escribía el gestor de archivos en **su**
/// directorio de datos y pasa a estar en la caché compartida, porque es dato
/// derivado —se rehace recorriendo el disco— y porque `~/.cache/vasak/` ya es
/// donde vive lo que no es de una sola aplicación.
///
/// Las dos rutas conviven a propósito. Un paquete no actualiza las dos
/// aplicaciones en el mismo instante, y acá el que pierde es siempre el mismo:
/// si el lanzador mira sólo la ruta nueva y el gestor todavía escribe en la
/// vieja, `abrir()` devuelve `None`, el proveedor no aporta filas y **el
/// lanzador sigue andando perfecto**. Nadie se entera hasta que alguien busca
/// un archivo y no aparece. Probar las dos hace que no haya día de corte.
///
/// La vieja se saca cuando ya no le sirva a nadie; hasta entonces es la
/// diferencia entre una transición invisible y una ventana de días en que la
/// búsqueda de archivos no encuentra nada sin decirlo.
///
/// La versión va en la ruta para que subirla descarte lo viejo solo. Al lado,
/// fuera del directorio de la versión, el gestor deja un `status.json` con el
/// número de esquema y la fecha del último escaneo: es lo que va a permitir
/// distinguir «todavía no hay índice» de «hay uno y es de otra versión», que
/// desde acá se ven igual. Leerlo es el paso siguiente y no está hecho.
const EN_LA_CACHE: &str = "vasak/global-search/v1/index";

/// Donde lo dejaba el gestor: el `identifier` de su `tauri.conf.json`, que es
/// lo que Tauri usa para el directorio de datos de cada aplicación.
const EN_LOS_DATOS: &str = "ar.net.vasak.vasak-file-manager/global-search/index";

/// Los campos que hacen falta acá, con el nombre que les puso el gestor.
const CAMPO_RUTA: &str = "path";
const CAMPO_NOMBRE: &str = "name";
const CAMPO_ES_DIRECTORIO: &str = "is_dir";

/// Cuánto vale un archivo frente a una aplicación.
///
/// Bastante menos: quien escribe «terminal» quiere la terminal, no un archivo
/// que se llame así. Los archivos son muchos y las aplicaciones pocas, y una
/// lista de resultados tomada por archivos no sirve para lanzar nada.
const PESO: f64 = 0.55;

/// Cuántos trae del índice antes de puntuar.
///
/// Tantivy ordena por su propia relevancia, que no es la misma que la de acá:
/// se piden unos cuantos y se reordenan con el puntaje del lanzador, que es el
/// que sabe que un nombre exacto vale más que uno parecido.
const DEL_INDICE: usize = 60;

const ICONO_ARCHIVO: &str = "text-x-generic";
const ICONO_DIRECTORIO: &str = "folder";

/// La base de un directorio del estándar, con su respaldo.
fn base(variable: &str, respaldo: &str) -> Option<PathBuf> {
    base_desde(
        std::env::var_os(variable).as_deref(),
        std::env::var_os("HOME").as_deref(),
        respaldo,
    )
}

/// Absoluta o nada.
///
/// El estándar pide que estas variables sean absolutas y que una relativa se
/// **ignore**, y acá además importa por lo que pasaría si no: una base relativa
/// haría que el índice se buscara respecto del directorio de trabajo, que en un
/// daemon lanzado por systemd no es el home de nadie. La búsqueda de archivos
/// dependería de desde dónde arrancó el proceso.
///
/// Una regla y no dos: la variable vacía es un caso de la misma, porque la
/// cadena vacía tampoco es absoluta. Antes estaba tratada aparte y la relativa
/// se colaba, que es el mismo agujero con otra forma.
///
/// Ignorar no es fallar: una `XDG_CACHE_HOME` relativa cae al respaldo, como si
/// no estuviera. Sin `HOME` absoluto no queda de dónde, y ahí sí es `None`.
fn base_desde(
    valor: Option<&std::ffi::OsStr>,
    home: Option<&std::ffi::OsStr>,
    respaldo: &str,
) -> Option<PathBuf> {
    if let Some(suya) = valor.map(Path::new).filter(|ruta| ruta.is_absolute()) {
        return Some(suya.to_path_buf());
    }

    let home = Path::new(home?);
    home.is_absolute().then(|| home.join(respaldo))
}

/// Las rutas donde puede estar el índice, en el orden en que hay que probarlas.
///
/// Primero la nueva: durante la transición pueden existir las dos, y la vieja
/// va a quedar congelada en lo que tuviera el día que el gestor dejó de
/// escribirla. Servir eso teniendo al lado una al día sería peor que no tener
/// respaldo.
pub fn rutas_del_indice() -> Vec<PathBuf> {
    rutas_desde(
        base("XDG_CACHE_HOME", ".cache").as_deref(),
        base("XDG_DATA_HOME", ".local/share").as_deref(),
    )
}

/// El orden, sin leer el entorno.
///
/// Aparte para poder probarlo: el entorno es global al proceso y las pruebas
/// corren en paralelo, así que una que lo toque decide el resultado de otra.
fn rutas_desde(cache: Option<&Path>, datos: Option<&Path>) -> Vec<PathBuf> {
    let mut rutas = Vec::new();

    if let Some(cache) = cache {
        rutas.push(cache.join(EN_LA_CACHE));
    }

    if let Some(datos) = datos {
        rutas.push(datos.join(EN_LOS_DATOS));
    }

    rutas
}

/// El índice abierto, o nada si no hay uno que se entienda.
pub struct Indice {
    lector: IndexReader,
    ruta: tantivy::schema::Field,
    nombre: tantivy::schema::Field,
    es_directorio: Option<tantivy::schema::Field>,
}

impl Indice {
    /// Abre el índice de sólo lectura.
    ///
    /// Devuelve `None` en todos los casos normales: el gestor no está
    /// instalado, nunca se escaneó, o el esquema cambió y ya no tiene los
    /// campos que hacen falta. Ninguno de los tres es un error que mostrar.
    pub fn abrir(ruta: &Path) -> Option<Self> {
        if !ruta.is_dir() {
            return None;
        }

        let indice = Index::open_in_dir(ruta).ok()?;
        let esquema = indice.schema();

        let campo_ruta = esquema.get_field(CAMPO_RUTA).ok()?;
        let campo_nombre = esquema.get_field(CAMPO_NOMBRE).ok()?;
        // Éste se puede perder sin que deje de servir: sin él, todo se muestra
        // con el icono de archivo.
        let campo_es_directorio = esquema.get_field(CAMPO_ES_DIRECTORIO).ok();

        let lector = indice.reader().ok()?;

        Some(Self {
            lector,
            ruta: campo_ruta,
            nombre: campo_nombre,
            es_directorio: campo_es_directorio,
        })
    }

    /// El índice del primer lugar donde haya uno que se entienda.
    ///
    /// «Que se entienda» y no «que exista»: un directorio con un índice de un
    /// esquema que no tiene los campos que hacen falta no sirve, y si se
    /// aceptara por estar primero taparía a uno bueno que esté más abajo.
    pub fn del_lugar_de_siempre() -> Option<Self> {
        Self::del_primero(&rutas_del_indice())
    }

    /// El primero de la lista que abra.
    ///
    /// **El primero que abre, no los dos.** Unir lo que haya en las dos rutas
    /// serviría duplicados y, peor, mezclaría los de la ruta vieja —que quedó
    /// congelada el día que el gestor dejó de escribirla— con los de la nueva,
    /// sin que se note cuáles son cuáles.
    fn del_primero(rutas: &[PathBuf]) -> Option<Self> {
        rutas.iter().find_map(|ruta| Self::abrir(ruta))
    }

    /// Los archivos que coinciden con lo escrito.
    pub fn buscar(&self, consulta: &str, limite: usize) -> Vec<Resultado> {
        let consulta = consulta.trim();
        if consulta.is_empty() {
            return Vec::new();
        }

        // Se relee en cada consulta: el gestor puede haber escrito mientras
        // tanto, y esto es lo que hace que el lector vea lo nuevo sin reabrir
        // el índice.
        if self.lector.reload().is_err() {
            return Vec::new();
        }

        let buscador = self.lector.searcher();
        let palabras: Vec<&str> = consulta
            .split(|c: char| c.is_whitespace() || c == '.' || c == '_' || c == '-')
            .filter(|parte| !parte.is_empty())
            .collect();

        if palabras.is_empty() {
            return Vec::new();
        }

        // Con tolerancia a un error de tipeo, que es lo que el gestor usa por
        // omisión y lo que uno espera al escribir un nombre de archivo de
        // memoria.
        let partes: Vec<(Occur, Box<dyn Query>)> = palabras
            .iter()
            .map(|palabra| {
                let termino = Term::from_field_text(self.nombre, &palabra.to_lowercase());
                let difusa: Box<dyn Query> = Box::new(FuzzyTermQuery::new(termino, 1, true));
                (Occur::Should, difusa)
            })
            .collect();

        let Ok(encontrados) =
            buscador.search(&BooleanQuery::new(partes), &TopDocs::with_limit(DEL_INDICE))
        else {
            return Vec::new();
        };

        let mut filas: Vec<Resultado> = encontrados
            .into_iter()
            .filter_map(|(_, direccion)| {
                let documento: TantivyDocument = buscador.doc(direccion).ok()?;
                self.como_resultado(&documento, consulta)
            })
            .collect();

        filas.sort_by(|a, b| {
            b.puntaje
                .partial_cmp(&a.puntaje)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.titulo.cmp(&b.titulo))
        });
        filas.truncate(limite);
        filas
    }

    fn como_resultado(&self, documento: &TantivyDocument, consulta: &str) -> Option<Resultado> {
        let ruta = documento.get_first(self.ruta)?.as_str()?.to_string();
        let nombre = documento
            .get_first(self.nombre)
            .and_then(|valor| valor.as_str())
            .map(str::to_string)
            .or_else(|| {
                Path::new(&ruta)
                    .file_name()
                    .map(|nombre| nombre.to_string_lossy().to_string())
            })?;

        let es_directorio = self
            .es_directorio
            .and_then(|campo| documento.get_first(campo))
            .and_then(|valor| valor.as_u64())
            .map(|valor| valor == 1)
            .unwrap_or(false);

        // El puntaje es el del lanzador y no el de tantivy: el de tantivy ordena
        // por relevancia de texto completo, que para un nombre de archivo no es
        // lo mismo que «se parece a lo que escribí».
        let suyo = puntaje::puntaje(consulta, &nombre) * PESO;

        (suyo > 0.0).then(|| Resultado {
            id: ruta.clone(),
            accion: None,
            titulo: nombre,
            subtitulo: Some(ruta),
            subtitulo_dato: None,
            icono: Some(
                if es_directorio {
                    ICONO_DIRECTORIO
                } else {
                    ICONO_ARCHIVO
                }
                .to_string(),
            ),
            puntaje: suyo,
            origen: Origen::Archivo,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tantivy::schema::{Schema, FAST, STORED, STRING, TEXT};

    /// Un índice con el mismo esquema que el del gestor, para probar el lector
    /// sin depender de que el gestor esté instalado ni de que alguien haya
    /// escaneado.
    fn indice_de_prueba(nombre: &str, archivos: &[(&str, &str, u64)]) -> PathBuf {
        let ruta =
            std::env::temp_dir().join(format!("prism-indice-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(&ruta).unwrap();

        let mut constructor = Schema::builder();
        let campo_ruta = constructor.add_text_field("path", STRING | STORED);
        let campo_nombre = constructor.add_text_field("name", TEXT | STORED);
        let campo_es_dir = constructor.add_u64_field("is_dir", FAST | STORED);
        let esquema = constructor.build();

        let indice = Index::create_in_dir(&ruta, esquema).unwrap();
        let mut escritor = indice.writer(15_000_000).unwrap();

        for (ruta_archivo, nombre_archivo, es_dir) in archivos {
            escritor
                .add_document(tantivy::doc!(
                    campo_ruta => *ruta_archivo,
                    campo_nombre => *nombre_archivo,
                    campo_es_dir => *es_dir
                ))
                .unwrap();
        }
        escritor.commit().unwrap();

        ruta
    }

    #[test]
    fn se_lee_el_indice_del_gestor_sin_ser_su_escritor() {
        let ruta = indice_de_prueba(
            "lectura",
            &[
                ("/home/pato/notas.md", "notas.md", 0),
                ("/home/pato/Documentos", "Documentos", 1),
            ],
        );

        let indice = Indice::abrir(&ruta).expect("el índice se abre");
        let filas = indice.buscar("notas", 10);

        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].titulo, "notas.md");
        assert_eq!(filas[0].id, "/home/pato/notas.md");
        assert_eq!(filas[0].origen, Origen::Archivo);

        let _ = std::fs::remove_dir_all(&ruta);
    }

    #[test]
    fn un_directorio_se_muestra_como_directorio() {
        let ruta = indice_de_prueba("carpeta", &[("/home/pato/Documentos", "Documentos", 1)]);
        let indice = Indice::abrir(&ruta).unwrap();

        assert_eq!(
            indice.buscar("documentos", 10)[0].icono.as_deref(),
            Some(ICONO_DIRECTORIO)
        );

        let _ = std::fs::remove_dir_all(&ruta);
    }

    #[test]
    fn valen_menos_que_una_aplicacion() {
        // Quien escribe «terminal» quiere la terminal, no un archivo así. Los
        // archivos son muchos y las aplicaciones pocas.
        let ruta = indice_de_prueba("peso", &[("/home/pato/terminal", "terminal", 0)]);
        let indice = Indice::abrir(&ruta).unwrap();

        assert!(indice.buscar("terminal", 10)[0].puntaje < 100.0);

        let _ = std::fs::remove_dir_all(&ruta);
    }

    #[test]
    fn con_el_campo_vacio_no_busca_nada() {
        let ruta = indice_de_prueba("vacio", &[("/home/pato/notas.md", "notas.md", 0)]);
        let indice = Indice::abrir(&ruta).unwrap();

        assert!(indice.buscar("", 10).is_empty());
        assert!(indice.buscar("   ", 10).is_empty());

        let _ = std::fs::remove_dir_all(&ruta);
    }

    #[test]
    fn sin_indice_no_hay_nada_que_abrir() {
        // El gestor no instalado, o nunca escaneado. No es un error.
        let inexistente = std::env::temp_dir().join("prism-indice-que-no-existe");
        assert!(Indice::abrir(&inexistente).is_none());
    }

    #[test]
    fn un_esquema_sin_los_campos_que_hacen_falta_no_se_usa() {
        // Si el gestor cambia el esquema, acá dejan de aparecer archivos — y eso
        // tiene que ser silencioso, no un lanzador que no abre.
        let ruta = std::env::temp_dir().join(format!("prism-esquema-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(&ruta).unwrap();

        let mut constructor = Schema::builder();
        constructor.add_text_field("otra_cosa", TEXT | STORED);
        Index::create_in_dir(&ruta, constructor.build()).unwrap();

        assert!(Indice::abrir(&ruta).is_none());

        let _ = std::fs::remove_dir_all(&ruta);
    }

    #[test]
    fn la_cache_va_antes_que_los_datos() {
        // El índice se mudó de los datos del gestor a la caché compartida. Si
        // el orden se diera vuelta, una instalación con las dos serviría la
        // vieja —congelada el día que el gestor dejó de escribirla— teniendo al
        // lado una al día.
        let rutas = rutas_desde(Some(Path::new("/c")), Some(Path::new("/d")));

        assert_eq!(
            rutas,
            vec![
                PathBuf::from("/c/vasak/global-search/v1/index"),
                PathBuf::from("/d/ar.net.vasak.vasak-file-manager/global-search/index"),
            ]
        );
    }

    #[test]
    fn la_ruta_nueva_es_la_que_acordamos_con_el_gestor() {
        // Clavada a propósito: es un contrato entre dos aplicaciones que se
        // actualizan por separado, y del lado de allá hay una prueba igual. Si
        // alguien la cambia de un solo lado, que falle acá y no en silencio.
        let rutas = rutas_desde(Some(Path::new("/c")), None);
        assert_eq!(
            rutas,
            vec![PathBuf::from("/c/vasak/global-search/v1/index")]
        );
    }

    #[test]
    fn sin_directorio_no_se_inventa_una_ruta_relativa() {
        // Un `XDG_CACHE_HOME=` vacío haría `join` sobre nada y daría una ruta
        // relativa al directorio de trabajo, que en un daemon es cualquier lado.
        assert!(rutas_desde(None, None).is_empty());
        assert_eq!(rutas_desde(None, Some(Path::new("/d"))).len(), 1);
    }

    #[test]
    fn se_abre_la_vieja_mientras_la_nueva_no_este() {
        // La transición: el gestor todavía escribe donde escribía. Sin esto el
        // proveedor no aporta filas y el lanzador sigue andando perfecto, que
        // es la peor forma de romperse.
        let vieja = indice_de_prueba(
            "transicion-vieja",
            &[("/home/pato/viejo.md", "viejo.md", 0)],
        );
        let nueva = std::env::temp_dir().join(format!("prism-no-esta-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&nueva);

        let indice = Indice::del_primero(&[nueva, vieja]).expect("tendría que abrir la vieja");
        assert_eq!(indice.buscar("viejo", 10).len(), 1);
    }

    #[test]
    fn con_las_dos_gana_la_nueva_y_la_vieja_no_se_mezcla() {
        // Unir las dos serviría duplicados y mezclaría lo congelado con lo que
        // está al día, sin que se note cuál es cuál.
        let nueva = indice_de_prueba("gana-nueva", &[("/home/pato/nuevo.md", "nuevo.md", 0)]);
        let vieja = indice_de_prueba("pierde-vieja", &[("/home/pato/viejo.md", "viejo.md", 0)]);

        let indice = Indice::del_primero(&[nueva, vieja]).expect("tendría que abrir");
        assert_eq!(indice.buscar("nuevo", 10).len(), 1);
        assert!(
            indice.buscar("viejo", 10).is_empty(),
            "la vieja no tiene que aportar nada cuando está la nueva"
        );
    }

    #[test]
    fn sin_ninguna_de_las_dos_no_hay_indice() {
        // El gestor no está instalado, o nunca se escaneó. No es un error.
        let ni = std::env::temp_dir().join(format!("prism-ni-una-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ni);

        assert!(Indice::del_primero(&[ni.join("a"), ni.join("b")]).is_none());
        assert!(Indice::del_primero(&[]).is_none());
    }

    #[test]
    fn una_base_relativa_se_ignora() {
        // El estándar pide ignorarla, y acá además importa por lo que pasaría:
        // el índice se buscaría respecto del directorio de trabajo, que en un
        // daemon lanzado por systemd no es el home de nadie.
        let home = Some(std::ffi::OsStr::new("/home/pato"));

        for relativa in ["relativa", "./relativa", "", "../arriba"] {
            assert_eq!(
                base_desde(Some(std::ffi::OsStr::new(relativa)), home, ".cache"),
                Some(PathBuf::from("/home/pato/.cache")),
                "«{relativa}» tendría que caer al respaldo"
            );
        }
    }

    #[test]
    fn una_base_absoluta_se_usa_tal_cual() {
        assert_eq!(
            base_desde(
                Some(std::ffi::OsStr::new("/tmp/cache")),
                Some(std::ffi::OsStr::new("/home/pato")),
                ".cache"
            ),
            Some(PathBuf::from("/tmp/cache"))
        );
    }

    #[test]
    fn sin_un_home_absoluto_no_hay_base() {
        // Devolver algo relativo sería peor que no devolver nada: `abrir()`
        // probaría una ruta que depende de dónde arrancó el proceso, y podría
        // hasta encontrar algo que no es el índice.
        assert_eq!(base_desde(None, None, ".cache"), None);
        assert_eq!(
            base_desde(None, Some(std::ffi::OsStr::new("casa")), ".cache"),
            None
        );
        assert_eq!(
            base_desde(None, Some(std::ffi::OsStr::new("")), ".cache"),
            None
        );
    }

    #[test]
    fn ninguna_ruta_candidata_es_relativa() {
        // El invariante de arriba, visto desde donde importa.
        for ruta in rutas_del_indice() {
            assert!(ruta.is_absolute(), "{ruta:?} es relativa");
        }
    }
}
