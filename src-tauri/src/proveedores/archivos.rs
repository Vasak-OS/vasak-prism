//! Los archivos, del índice que el lanzador escanea y mantiene.
//!
//! # Quién es el dueño del índice
//!
//! Desde la 0.13, este programa. Lo escribía `vasak-file-manager` y sólo cuando
//! alguien abría su ventana y lo pedía; el lanzador lo leía de prestado. Eso
//! resolvía el «qué buscar» y dejaba abierto el «cuándo se actualiza»: un
//! archivo bajado hace diez minutos no estaba, y nada decía por qué.
//!
//! El escaneo vive acá porque acá está el proceso que vive prendido —unidad de
//! systemd, `Type=dbus`, con la sesión—, y el gestor es una ventana que se abre
//! y se cierra. Ver Vasak-OS/vasak-prism#33; el escaneo está en [`escaneo`] y
//! lo que los dos programas comparten, en [`contrato`].
//!
//! **La ruta no cambió.** El índice ya vivía en la caché compartida, que no
//! cuelga del nombre de ninguna de las dos aplicaciones, así que cambiar de
//! dueño no movió ni rehizo nada: el índice que hubiera se hereda tal cual.
//!
//! # Leer y escribir conviven
//!
//! Tantivy admite **muchos lectores y un escritor**, y los lectores no
//! necesitan que el escritor esté vivo. Este archivo es el lector; el de al
//! lado, el escritor. Mientras haya instalaciones con el gestor sin actualizar,
//! el escritor puede ser el otro: el bloqueo de tantivy hace que no haya dos a
//! la vez, y el que llega segundo no hace nada. Por eso el cambio de dueño no
//! necesita que las dos aplicaciones se actualicen en el mismo instante.
//!
//! # Nada de esto falla ruidosamente
//!
//! Sin índice, o con uno que no se entiende, no hay filas y el resto del
//! lanzador anda igual. Es deliberado, y tiene su costo: el día que el esquema
//! cambie de un lado y no del otro, acá dejan de aparecer archivos **sin ningún
//! error**. Contra eso está [`contrato`], con una prueba que fija los nombres
//! de los campos y la ruta de los dos lados.

pub mod contrato;
pub mod escaneo;

use std::path::{Path, PathBuf};

use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, FuzzyTermQuery, Occur, Query};
use tantivy::schema::Value;
use tantivy::{Index, IndexReader, TantivyDocument, Term};

use crate::catalogo::aplicacion::{Origen, Resultado};
use crate::catalogo::puntaje;

/// Donde el gestor dejaba el índice antes de que se mudara a la caché
/// compartida: el `identifier` de su `tauri.conf.json`, que es lo que Tauri usa
/// para el directorio de datos de cada aplicación.
///
/// # Por qué se sigue mirando
///
/// Porque un paquete no actualiza las dos aplicaciones en el mismo instante.
/// Mientras haya instalaciones con el gestor viejo —el que escribía acá— ésta
/// es la única ruta con algo adentro, y mirarla es la diferencia entre una
/// transición invisible y unos días en que la búsqueda de archivos no encuentra
/// nada **sin decir por qué**: `abrir()` devuelve `None`, el proveedor no
/// aporta filas y el lanzador sigue andando perfecto.
///
/// Se saca cuando ya no le sirva a nadie.
const EN_LOS_DATOS: &str = "ar.net.vasak.vasak-file-manager/global-search/index";

/// Los campos que hacen falta acá. Los nombres salen del contrato y no de una
/// constante propia: son lo que los dos programas comparten.
use contrato::{CAMPO_ES_DIRECTORIO, CAMPO_NOMBRE, CAMPO_RUTA};

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

/// Las rutas donde puede estar el índice, en el orden en que hay que probarlas.
///
/// Primero la nueva: durante la transición pueden existir las dos, y la vieja
/// va a quedar congelada en lo que tuviera el día que el gestor dejó de
/// escribirla. Servir eso teniendo al lado una al día sería peor que no tener
/// respaldo.
pub fn rutas_del_indice() -> Vec<PathBuf> {
    rutas_desde(
        contrato::base_de_cache().as_deref(),
        crate::rutas::base(dirs::data_dir()).as_deref(),
    )
}

/// El orden, sin leer el entorno.
///
/// Aparte para poder probarlo: el entorno es global al proceso y las pruebas
/// corren en paralelo, así que una que lo toque decide el resultado de otra.
fn rutas_desde(compartida: Option<&Path>, datos: Option<&Path>) -> Vec<PathBuf> {
    let mut rutas = Vec::new();

    if let Some(compartida) = compartida {
        rutas.push(contrato::directorio_del_indice(compartida));
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
        let rutas = rutas_desde(Some(Path::new("/c/vasak")), Some(Path::new("/d")));

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
        let rutas = rutas_desde(Some(Path::new("/c/vasak")), None);
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
    fn ninguna_ruta_candidata_es_relativa() {
        // El invariante de arriba, visto desde donde importa.
        for ruta in rutas_del_indice() {
            assert!(ruta.is_absolute(), "{ruta:?} es relativa");
        }
    }
}
