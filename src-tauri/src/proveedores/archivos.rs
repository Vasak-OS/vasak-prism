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

/// Dónde deja el gestor de archivos su índice, relativo al directorio de datos.
///
/// El nombre del directorio es el `identifier` de su `tauri.conf.json`, que es
/// lo que Tauri usa para el directorio de datos de cada aplicación.
const INDICE: &str = "ar.net.vasak.vasak-file-manager/global-search/index";

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

pub fn ruta_del_indice() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(valor) if !valor.is_empty() => PathBuf::from(valor),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/share"),
    };

    Some(base.join(INDICE))
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

    pub fn del_lugar_de_siempre() -> Option<Self> {
        Self::abrir(&ruta_del_indice()?)
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
}
