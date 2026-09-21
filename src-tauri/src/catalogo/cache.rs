//! La caché del índice de aplicaciones en disco.
//!
//! Existe para que la primera consulta después de arrancar no espere a que se
//! lean quinientos archivos. El proceso levanta con la sesión, lee la caché
//! —una consulta a una base de unos pocos cientos de kilobytes— y ya puede
//! contestar; la revalidación va después y en otro hilo.
//!
//! No es la fuente de la verdad: si no está, si está vieja o si no se entiende,
//! se escanea el disco y listo. Ninguna función de acá devuelve un error que
//! haya que mostrarle a alguien — como mucho, se pierde el atajo.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension};

use super::aplicacion::Aplicacion;
use super::escaneo::Archivo;

/// La versión del esquema. Subirla descarta lo guardado, que es justo lo que
/// hay que hacer cuando cambia la forma de una fila: leer una fila vieja con el
/// código nuevo da una aplicación a medio armar, y eso no se nota hasta que
/// alguien la elige.
const ESQUEMA: i64 = 1;

/// Dónde vive la caché: `$XDG_CACHE_HOME/vasak-prism/aplicaciones.db`.
///
/// En caché y no en datos: es contenido derivado del disco, se puede rehacer
/// entero en cualquier momento, y no tiene por qué sobrevivir a un borrado ni
/// entrar en una copia de respaldo.
pub fn ruta_por_defecto() -> Option<PathBuf> {
    // Por `dirs` y no leyendo el entorno acá: trataba la variable **vacía** y
    // no la **relativa**, que tiene la misma consecuencia —una ruta respecto
    // del directorio de trabajo, que en un daemon de systemd es cualquier
    // lado—. `dirs` lo cubre con una sola regla, porque la cadena vacía tampoco
    // es absoluta. Sobre `HOME` sólo mira que no esté vacía, así que el filtro
    // cierra esa otra mitad.
    let base = crate::rutas::base(dirs::cache_dir())?;

    Some(base.join("vasak-prism").join("aplicaciones.db"))
}

pub struct Cache {
    conexion: Connection,
}

impl Cache {
    /// Abre —o crea— la caché en esa ruta.
    pub fn abrir(ruta: &Path) -> Result<Self, String> {
        if let Some(directorio) = ruta.parent() {
            std::fs::create_dir_all(directorio).map_err(|e| e.to_string())?;
        }

        let conexion = Connection::open(ruta).map_err(|e| e.to_string())?;
        let cache = Self { conexion };
        cache.preparar()?;
        Ok(cache)
    }

    /// En memoria. Para las pruebas, que no tienen por qué tocar el disco.
    #[cfg(test)]
    pub fn en_memoria() -> Result<Self, String> {
        let conexion = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let cache = Self { conexion };
        cache.preparar()?;
        Ok(cache)
    }

    fn preparar(&self) -> Result<(), String> {
        self.conexion
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS meta (clave TEXT PRIMARY KEY, valor TEXT NOT NULL);
                 CREATE TABLE IF NOT EXISTS aplicaciones (
                     id TEXT PRIMARY KEY,
                     datos TEXT NOT NULL,
                     mtime INTEGER NOT NULL
                 );",
            )
            .map_err(|e| e.to_string())?;

        let guardado: Option<i64> = self
            .conexion
            .query_row(
                "SELECT valor FROM meta WHERE clave = 'esquema'",
                [],
                |fila| fila.get::<_, String>(0),
            )
            .optional()
            .map_err(|e| e.to_string())?
            .and_then(|valor| valor.parse().ok());

        if guardado != Some(ESQUEMA) {
            self.conexion
                .execute("DELETE FROM aplicaciones", [])
                .map_err(|e| e.to_string())?;
            self.conexion
                .execute(
                    "INSERT OR REPLACE INTO meta (clave, valor) VALUES ('esquema', ?1)",
                    [ESQUEMA.to_string()],
                )
                .map_err(|e| e.to_string())?;
        }

        Ok(())
    }

    /// Lo guardado. Una fila que no se entiende se saltea en lugar de tirar todo
    /// abajo: una aplicación de menos es mejor que un lanzador vacío.
    pub fn leer(&self) -> Vec<Aplicacion> {
        let Ok(mut consulta) = self
            .conexion
            .prepare("SELECT datos FROM aplicaciones ORDER BY id")
        else {
            return Vec::new();
        };

        let filas = consulta.query_map([], |fila| fila.get::<_, String>(0));

        let Ok(filas) = filas else {
            return Vec::new();
        };

        filas
            .flatten()
            .filter_map(|datos| serde_json::from_str(&datos).ok())
            .collect()
    }

    /// Reemplaza lo guardado por esto, en una sola transacción.
    ///
    /// En una sola porque son quinientas escrituras: de a una, cada `INSERT` es
    /// su propia transacción y su propio `fsync`, y lo que tarda medio segundo
    /// pasa a tardar medio minuto en un disco mecánico.
    pub fn guardar(
        &mut self,
        aplicaciones: &[Aplicacion],
        ultimo_visto: i64,
    ) -> Result<(), String> {
        let transaccion = self.conexion.transaction().map_err(|e| e.to_string())?;

        transaccion
            .execute("DELETE FROM aplicaciones", [])
            .map_err(|e| e.to_string())?;

        {
            let mut insertar = transaccion
                .prepare("INSERT INTO aplicaciones (id, datos, mtime) VALUES (?1, ?2, ?3)")
                .map_err(|e| e.to_string())?;

            for aplicacion in aplicaciones {
                let datos = serde_json::to_string(aplicacion).map_err(|e| e.to_string())?;
                insertar
                    .execute(rusqlite::params![aplicacion.id, datos, aplicacion.mtime])
                    .map_err(|e| e.to_string())?;
            }
        }

        transaccion
            .execute(
                "INSERT OR REPLACE INTO meta (clave, valor) VALUES ('ultimo_visto', ?1)",
                [ultimo_visto.to_string()],
            )
            .map_err(|e| e.to_string())?;

        transaccion.commit().map_err(|e| e.to_string())
    }

    /// La fecha del archivo más nuevo que vio el escaneo que guardó esto.
    ///
    /// Cero cuando no está, que es el caso de una caché escrita por una versión
    /// anterior. Con cero, la primera comprobación da «vieja» y se reindexa una
    /// vez; de ahí en más la marca queda escrita y la cosa se estabiliza. Es un
    /// reindexado de más contra el que había antes, que era uno por arranque
    /// para siempre.
    pub fn ultimo_visto(&self) -> i64 {
        self.conexion
            .query_row(
                "SELECT valor FROM meta WHERE clave = 'ultimo_visto'",
                [],
                |fila| fila.get::<_, String>(0),
            )
            .ok()
            .and_then(|valor| valor.parse().ok())
            .unwrap_or(0)
    }
}

/// Si lo guardado sigue valiendo para los archivos que hay en el disco.
///
/// Compara identificador y fecha de modificación, no sólo la cantidad: cambiar
/// un `.desktop` sin agregar ni sacar ninguno es lo que pasa al actualizar un
/// paquete, y es justo el caso que una comprobación por cantidad deja pasar.
///
/// No alcanza con mirar la fecha de los directorios: un archivo editado en su
/// lugar no cambia la del directorio que lo contiene.
pub fn esta_al_dia(guardadas: &[Aplicacion], archivos: &[Archivo], ultimo_visto: i64) -> bool {
    // Las entradas que el escaneo descarta —`NoDisplay`, de otro escritorio, sin
    // programa— están en el disco y no en la caché, así que no se pueden contar
    // de los dos lados. Lo que se comprueba es que ninguna de las guardadas haya
    // cambiado, y que ningún archivo sea más nuevo que el más nuevo que **se
    // miró** al guardar.
    //
    // Que la marca venga de lo mirado y no de lo guardado es el punto. Antes
    // salía del `mtime` más alto de las aplicaciones guardadas, y entonces un
    // `.desktop` descartado que fuera el más nuevo del disco se leía como una
    // aplicación recién instalada: la caché se regeneraba, el archivo seguía
    // descartado y seguía siendo el más nuevo, y la próxima comprobación volvía
    // a decir lo mismo. El catálogo se reindexaba entero en cada arranque y
    // nada lo decía.
    let en_cache: HashMap<&str, i64> = guardadas
        .iter()
        .map(|app| (app.id.as_str(), app.mtime))
        .collect();

    for archivo in archivos {
        match en_cache.get(archivo.id.as_str()) {
            // Estaba: tiene que estar igual.
            Some(mtime) if *mtime == archivo.mtime => {}
            Some(_) => return false,
            // No estaba. Puede ser una entrada que el escaneo descarta y
            // entonces está bien que falte, o una nueva. La fecha lo dice: una
            // nueva es más nueva que todo lo que había.
            None if archivo.mtime <= ultimo_visto => {}
            None => return false,
        }
    }

    // Y al revés: una aplicación guardada cuyo archivo ya no está.
    let en_disco: HashMap<&str, i64> = archivos
        .iter()
        .map(|archivo| (archivo.id.as_str(), archivo.mtime))
        .collect();

    guardadas
        .iter()
        .all(|app| en_disco.contains_key(app.id.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn una(id: &str, mtime: i64) -> Aplicacion {
        Aplicacion {
            id: id.to_string(),
            nombre: id.to_string(),
            generico: None,
            comentario: None,
            palabras: Vec::new(),
            icono: None,
            exec: "x".to_string(),
            terminal: false,
            acciones: Vec::new(),
            ruta: format!("/usr/share/applications/{id}"),
            mtime,
        }
    }

    fn archivo(id: &str, mtime: i64) -> Archivo {
        Archivo {
            id: id.to_string(),
            ruta: PathBuf::from(format!("/usr/share/applications/{id}")),
            mtime,
        }
    }

    #[test]
    fn lo_guardado_se_lee_igual() {
        let mut cache = Cache::en_memoria().unwrap();
        let mut firefox = una("firefox.desktop", 10);
        firefox.palabras = vec!["web".to_string()];
        firefox.comentario = Some("Navegá la web".to_string());

        cache.guardar(&[firefox.clone()], 10).unwrap();

        assert_eq!(cache.leer(), vec![firefox]);
    }

    #[test]
    fn guardar_reemplaza_y_no_acumula() {
        // Sin el borrado, cada escaneo dejaba las de antes y la lista crecía con
        // aplicaciones desinstaladas.
        let mut cache = Cache::en_memoria().unwrap();
        cache
            .guardar(&[una("a.desktop", 1), una("b.desktop", 1)], 1)
            .unwrap();
        cache.guardar(&[una("a.desktop", 2)], 2).unwrap();

        let leidas = cache.leer();
        assert_eq!(leidas.len(), 1);
        assert_eq!(leidas[0].mtime, 2);
    }

    #[test]
    fn una_cache_vacia_no_es_un_error() {
        assert!(Cache::en_memoria().unwrap().leer().is_empty());
    }

    #[test]
    fn sin_cambios_la_cache_vale() {
        let guardadas = [una("a.desktop", 5), una("b.desktop", 7)];
        let archivos = [archivo("a.desktop", 5), archivo("b.desktop", 7)];
        assert!(esta_al_dia(&guardadas, &archivos, 7));
    }

    #[test]
    fn un_archivo_editado_invalida_la_cache() {
        // Actualizar un paquete cambia el `.desktop` sin agregar ni sacar
        // ninguno: una comprobación por cantidad lo deja pasar.
        let guardadas = [una("a.desktop", 5)];
        let archivos = [archivo("a.desktop", 6)];
        assert!(!esta_al_dia(&guardadas, &archivos, 5));
    }

    #[test]
    fn una_aplicacion_nueva_invalida_la_cache() {
        let guardadas = [una("a.desktop", 5)];
        let archivos = [archivo("a.desktop", 5), archivo("b.desktop", 9)];
        assert!(!esta_al_dia(&guardadas, &archivos, 5));
    }

    #[test]
    fn una_aplicacion_borrada_invalida_la_cache() {
        let guardadas = [una("a.desktop", 5), una("b.desktop", 5)];
        let archivos = [archivo("a.desktop", 5)];
        assert!(!esta_al_dia(&guardadas, &archivos, 5));
    }

    #[test]
    fn las_entradas_que_el_escaneo_descarta_no_invalidan_nada() {
        // La mitad de los `.desktop` de un sistema son `NoDisplay`: están en el
        // disco y nunca van a estar en la caché. Si contaran, la caché estaría
        // vencida siempre y no serviría para nada.
        let guardadas = [una("a.desktop", 5)];
        let archivos = [archivo("a.desktop", 5), archivo("oculta.desktop", 3)];
        assert!(esta_al_dia(&guardadas, &archivos, 5));
    }

    #[test]
    fn la_descartada_mas_nueva_del_disco_no_vence_la_cache() {
        // El caso que hacía reindexar en cada arranque. `oculta.desktop` es
        // `NoDisplay`, así que nunca va a estar en la caché, y además es el
        // archivo más nuevo del disco.
        //
        // Antes la marca salía del `mtime` más alto de lo **guardado** —5— así
        // que la descartada, con 9, se leía como una aplicación recién
        // instalada. Y no se salía nunca: se regeneraba la caché, la entrada
        // seguía descartada, seguía siendo la más nueva, y la comprobación
        // siguiente decía lo mismo. Nada lo avisaba.
        //
        // Ahora la marca es la del archivo más nuevo que el escaneo **miró**,
        // descartadas incluidas, así que 9 no es novedad: ya se había visto.
        let guardadas = [una("a.desktop", 5)];
        let archivos = [archivo("a.desktop", 5), archivo("oculta.desktop", 9)];

        assert!(esta_al_dia(&guardadas, &archivos, 9));
    }

    #[test]
    fn una_aplicacion_nueva_despues_de_esa_marca_si_la_vence() {
        // La otra mitad, que es la que no hay que romper al arreglar lo de
        // arriba: subir la marca no puede hacer que una instalación de verdad
        // pase desapercibida.
        let guardadas = [una("a.desktop", 5)];
        let archivos = [
            archivo("a.desktop", 5),
            archivo("oculta.desktop", 9),
            archivo("nueva.desktop", 10),
        ];

        assert!(!esta_al_dia(&guardadas, &archivos, 9));
    }

    #[test]
    fn la_marca_se_guarda_y_se_relee() {
        // Sin persistirla, cada arranque empezaría con cero y la primera
        // comprobación diría «vieja» siempre: el mismo reindexado por arranque
        // con otra causa.
        let mut cache = Cache::en_memoria().unwrap();

        assert_eq!(cache.ultimo_visto(), 0, "una caché nueva no vio nada");

        cache.guardar(&[una("a.desktop", 5)], 9).unwrap();

        assert_eq!(cache.ultimo_visto(), 9);
    }

    #[test]
    fn una_cache_vacia_nunca_esta_al_dia_si_hay_archivos() {
        assert!(!esta_al_dia(&[], &[archivo("a.desktop", 1)], 0));
    }
}
