//! Dónde están las entradas del escritorio y cómo se recorren.
//!
//! Los directorios salen de `XDG_DATA_HOME` y `XDG_DATA_DIRS`, no de una lista
//! fija de tres rutas. La lista fija es lo que había, y deja afuera los
//! directorios que agregan al entorno los entornos de desarrollo, los perfiles
//! de sistema y `nix`: una aplicación instalada ahí no existe para el lanzador.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::aplicacion::Aplicacion;
use super::entrada;

/// Los directorios de aplicaciones, del más específico al más general.
///
/// El orden importa: el estándar dice que la primera entrada con un mismo
/// identificador gana, así que lo del usuario pisa lo del sistema. Es como se
/// personaliza una entrada sin tocar `/usr`.
pub fn directorios() -> Vec<PathBuf> {
    let mut rutas = Vec::new();

    if let Some(propio) = data_home() {
        rutas.push(propio.join("applications"));
    }

    for base in data_dirs() {
        rutas.push(base.join("applications"));
    }

    rutas.retain(|ruta| ruta.is_dir());
    rutas.dedup();
    rutas
}

fn data_home() -> Option<PathBuf> {
    // Ver `catalogo::cache::ruta_por_defecto`: la variable vacía estaba
    // cubierta y la relativa no, y las dos dan lo mismo.
    crate::rutas::base(dirs::data_dir())
}

fn data_dirs() -> Vec<PathBuf> {
    data_dirs_de(&std::env::var("XDG_DATA_DIRS").unwrap_or_default())
}

/// Separado de la lectura de la variable para poder probarlo.
///
/// Tocar el entorno adentro de una prueba es tocárselo a las que corren en
/// paralelo en el mismo proceso, y el fallo aparece en otra prueba y sin motivo
/// visible.
fn data_dirs_de(valor: &str) -> Vec<PathBuf> {
    // El de reserva que fija el estándar. Sin esto, un entorno que no exporta
    // la variable —una sesión mínima, un contenedor— se queda sin ninguna
    // aplicación del sistema.
    let valor = if valor.trim().is_empty() {
        "/usr/local/share:/usr/share"
    } else {
        valor
    };

    valor
        .split(':')
        .filter(|parte| !parte.is_empty())
        .map(PathBuf::from)
        .collect()
}

/// Un archivo `.desktop` encontrado, con lo que hace falta para saber si cambió.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Archivo {
    /// El identificador del estándar: la ruta relativa al directorio de
    /// aplicaciones, con las barras cambiadas por guiones.
    pub id: String,
    pub ruta: PathBuf,
    /// Segundos desde la época. Cero si el sistema de archivos no los da.
    pub mtime: i64,
}

/// Todos los archivos `.desktop` visibles, ya resuelta la precedencia.
///
/// Recursivo, porque el estándar lo es: `kde/konsole.desktop` existe y su
/// identificador es `kde-konsole.desktop`. Recorrer un solo nivel deja afuera
/// todo lo que instala KDE, que es de lo que más hay en un sistema mixto.
pub fn archivos() -> Vec<Archivo> {
    let mut encontrados: HashMap<String, Archivo> = HashMap::new();

    for directorio in directorios() {
        for archivo in recorrer(&directorio, &directorio) {
            // El primero gana: los directorios vienen del más específico al más
            // general, y `entry` deja el que ya estaba.
            encontrados.entry(archivo.id.clone()).or_insert(archivo);
        }
    }

    let mut salida: Vec<Archivo> = encontrados.into_values().collect();
    // Orden estable, para que dos escaneos del mismo disco den lo mismo: sin
    // esto la caché se ve distinta cada vez y las pruebas son una lotería.
    salida.sort_by(|a, b| a.id.cmp(&b.id));
    salida
}

fn recorrer(raiz: &Path, directorio: &Path) -> Vec<Archivo> {
    let Ok(entradas) = fs::read_dir(directorio) else {
        return Vec::new();
    };

    let mut salida = Vec::new();

    for entrada in entradas.flatten() {
        let ruta = entrada.path();

        if ruta.is_dir() {
            salida.extend(recorrer(raiz, &ruta));
            continue;
        }

        if ruta.extension().is_none_or(|ext| ext != "desktop") {
            continue;
        }

        let Some(id) = identificador(raiz, &ruta) else {
            continue;
        };

        salida.push(Archivo {
            id,
            mtime: modificado(&entrada),
            ruta,
        });
    }

    salida
}

/// El identificador del estándar: `kde/konsole.desktop` -> `kde-konsole.desktop`.
pub fn identificador(raiz: &Path, ruta: &Path) -> Option<String> {
    let relativa = ruta.strip_prefix(raiz).ok()?;
    Some(
        relativa
            .components()
            .map(|parte| parte.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("-"),
    )
}

fn modificado(entrada: &fs::DirEntry) -> i64 {
    entrada
        .metadata()
        .ok()
        .and_then(|datos| datos.modified().ok())
        .and_then(|momento| momento.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|desde| desde.as_secs() as i64)
        .unwrap_or(0)
}

/// Lee y parsea todos los archivos, y deja las que se pueden mostrar y lanzar.
pub fn escanear(idioma: &str, escritorios: &[String]) -> Vec<Aplicacion> {
    archivos()
        .into_iter()
        .filter_map(|archivo| leer(&archivo, idioma, escritorios))
        .collect()
}

/// Lee un archivo suelto. Devuelve `None` si no es una entrada para mostrar.
pub fn leer(archivo: &Archivo, idioma: &str, escritorios: &[String]) -> Option<Aplicacion> {
    let contenido = fs::read_to_string(&archivo.ruta).ok()?;
    let entrada = entrada::parsear(&contenido, &archivo.id, idioma, escritorios)?;

    // El `TryExec` se comprueba acá y no en el parser porque es lo único que
    // mira el disco: una entrada de un programa desinstalado aparece en la
    // lista, se elige, y no abre nada.
    if !entrada.puede_ejecutarse() {
        return None;
    }

    Some(Aplicacion::desde(entrada, &archivo.ruta, archivo.mtime))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_identificador_lleva_los_subdirectorios() {
        let raiz = Path::new("/usr/share/applications");
        assert_eq!(
            identificador(raiz, Path::new("/usr/share/applications/firefox.desktop")),
            Some("firefox.desktop".to_string())
        );
        // Lo que se pierde recorriendo un solo nivel: todo lo de KDE.
        assert_eq!(
            identificador(
                raiz,
                Path::new("/usr/share/applications/kde/konsole.desktop")
            ),
            Some("kde-konsole.desktop".to_string())
        );
    }

    #[test]
    fn una_ruta_de_otro_arbol_no_da_identificador() {
        assert_eq!(
            identificador(
                Path::new("/usr/share/applications"),
                Path::new("/otro/lado/x.desktop")
            ),
            None
        );
    }

    #[test]
    fn sin_xdg_data_dirs_quedan_los_del_estandar() {
        // Un entorno que no exporta la variable no puede quedarse sin las
        // aplicaciones del sistema.
        let rutas = data_dirs_de("");
        assert!(rutas.contains(&PathBuf::from("/usr/share")));
        assert!(rutas.contains(&PathBuf::from("/usr/local/share")));
        assert_eq!(data_dirs_de("   "), data_dirs_de(""));
    }

    #[test]
    fn los_directorios_del_entorno_se_respetan() {
        assert_eq!(
            data_dirs_de("/uno:/dos"),
            vec![PathBuf::from("/uno"), PathBuf::from("/dos")]
        );
        // Un separador de más no agrega un directorio vacío, que se leería como
        // el directorio actual.
        assert_eq!(data_dirs_de("/uno::"), vec![PathBuf::from("/uno")]);
    }
}
