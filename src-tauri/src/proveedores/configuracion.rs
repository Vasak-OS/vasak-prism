//! Las secciones de la configuración, como resultados.
//!
//! Escribís «wifi» y la fila abre la configuración **en** Wi-Fi, no en su
//! portada. Es la diferencia entre encontrar el ajuste y encontrar la ventana
//! donde el ajuste está en algún lado.
//!
//! La lista no está escrita acá: la publica `vasak-settings` en un archivo que
//! instala su paquete, generado desde su propio menú lateral y con una prueba
//! que no lo deja separarse. Copiarla sería tener dos listas, y la de acá
//! quedaría vieja la primera vez que alguien agregue una pantalla.

use std::path::PathBuf;

use serde::Deserialize;

use crate::catalogo::aplicacion::{Origen, Resultado};
use crate::catalogo::puntaje;

/// Dónde lo deja el paquete de la configuración, relativo a un directorio de
/// datos.
const ARCHIVO: &str = "vasak-settings/secciones.json";

/// Cuánto vale una sección frente a una aplicación.
///
/// Un poco menos: quien escribe «pantallas» y tiene una aplicación que se llama
/// así quiere la aplicación. Pero lo suficiente como para ganarle a una
/// coincidencia parcial.
const PESO: f64 = 0.9;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Seccion {
    /// Lo que se le pasa al programa: `vasak-settings network-wifi`.
    pub id: String,
    pub icono: String,
    /// El nombre en cada idioma, ya resuelto.
    pub nombres: std::collections::HashMap<String, String>,
}

impl Seccion {
    /// El nombre en el idioma de la sesión, o en el que haya.
    ///
    /// Caer al que haya y no a la clave: una sección con el nombre en inglés se
    /// puede leer y elegir; una que muestre `network-wifi` es una fila que
    /// nadie entiende.
    pub fn nombre(&self, idioma: &str) -> &str {
        self.nombres
            .get(idioma)
            .or_else(|| self.nombres.get("es"))
            .or_else(|| self.nombres.values().next())
            .map(String::as_str)
            .unwrap_or(&self.id)
    }
}

/// Dónde buscar el archivo, en el orden del estándar.
///
/// Por `XDG_DATA_DIRS` y no con la ruta clavada en `/usr/share`: así lo
/// encuentra también una instalación en otro prefijo, que es como se prueban
/// las dos aplicaciones juntas sin instalarlas en el sistema.
fn rutas() -> Vec<PathBuf> {
    let mut salida = Vec::new();

    if let Some(propio) = std::env::var_os("XDG_DATA_HOME").filter(|valor| !valor.is_empty()) {
        salida.push(PathBuf::from(propio).join(ARCHIVO));
    } else if let Some(home) = std::env::var_os("HOME") {
        salida.push(PathBuf::from(home).join(".local/share").join(ARCHIVO));
    }

    let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
    let dirs = if dirs.trim().is_empty() {
        "/usr/local/share:/usr/share".to_string()
    } else {
        dirs
    };

    for base in dirs.split(':').filter(|parte| !parte.is_empty()) {
        salida.push(PathBuf::from(base).join(ARCHIVO));
    }

    salida
}

/// Lee el catálogo. Vacío si la configuración no está instalada, que es un
/// caso normal y no un error: el lanzador funciona igual sin esas filas.
pub fn del_disco() -> Vec<Seccion> {
    for ruta in rutas() {
        let Ok(contenido) = std::fs::read_to_string(&ruta) else {
            continue;
        };
        if let Ok(leidas) = leer(&contenido) {
            return leidas;
        }
    }

    Vec::new()
}

pub fn leer(contenido: &str) -> Result<Vec<Seccion>, serde_json::Error> {
    serde_json::from_str(contenido)
}

/// Las secciones que coinciden con lo escrito.
pub fn buscar(
    secciones: &[Seccion],
    consulta: &str,
    idioma: &str,
    limite: usize,
) -> Vec<Resultado> {
    let consulta = consulta.trim();
    if consulta.is_empty() {
        return Vec::new();
    }

    let mut filas: Vec<Resultado> = secciones
        .iter()
        .filter_map(|seccion| {
            let nombre = seccion.nombre(idioma);
            let suyo = puntaje::puntaje(consulta, nombre) * PESO;

            (suyo > 0.0).then(|| Resultado {
                id: seccion.id.clone(),
                accion: None,
                titulo: nombre.to_string(),
                subtitulo: Some("lanzador.enLaConfiguracion".to_string()),
                subtitulo_dato: None,
                icono: Some(seccion.icono.clone()),
                puntaje: suyo,
                origen: Origen::Configuracion,
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOGO: &str = r#"[
        {"id": "network-wifi", "icono": "network-wireless", "nombres": {"es": "Wi-Fi", "en": "Wi-Fi"}},
        {"id": "monitors", "icono": "video-display", "nombres": {"es": "Pantallas", "en": "Displays"}},
        {"id": "appearance-fonts", "icono": "preferences-desktop-font", "nombres": {"es": "Fuentes", "en": "Fonts"}}
    ]"#;

    fn secciones() -> Vec<Seccion> {
        leer(CATALOGO).expect("el catálogo de prueba parsea")
    }

    #[test]
    fn se_lee_el_catalogo_que_publica_la_configuracion() {
        let leidas = secciones();
        assert_eq!(leidas.len(), 3);
        assert_eq!(leidas[0].id, "network-wifi");
        assert_eq!(leidas[0].nombre("es"), "Wi-Fi");
        assert_eq!(leidas[1].nombre("en"), "Displays");
    }

    #[test]
    fn se_busca_en_el_idioma_de_la_sesion() {
        assert_eq!(
            buscar(&secciones(), "pantallas", "es", 10)[0].id,
            "monitors"
        );
        assert_eq!(buscar(&secciones(), "displays", "en", 10)[0].id, "monitors");
    }

    #[test]
    fn un_idioma_que_no_esta_cae_a_uno_que_si() {
        // Una sección con el nombre en otro idioma se puede leer y elegir; una
        // que muestre `network-wifi` es una fila que nadie entiende.
        let leidas = secciones();
        assert_eq!(leidas[1].nombre("de"), "Pantallas");
    }

    #[test]
    fn el_guion_del_nombre_no_hace_falta_escribirlo() {
        // «Wi-Fi» se escribe «wifi». Sin esto, la sección más buscada de todas
        // no aparecía.
        assert_eq!(buscar(&secciones(), "wifi", "es", 10)[0].id, "network-wifi");
        assert_eq!(
            buscar(&secciones(), "wi-fi", "es", 10)[0].id,
            "network-wifi"
        );
    }

    #[test]
    fn valen_un_poco_menos_que_una_aplicacion() {
        // Quien escribe «pantallas» y tiene una aplicación que se llama así
        // quiere la aplicación.
        assert!(buscar(&secciones(), "pantallas", "es", 10)[0].puntaje < 100.0);
    }

    #[test]
    fn con_el_campo_vacio_no_aparecen() {
        assert!(buscar(&secciones(), "", "es", 10).is_empty());
    }

    #[test]
    fn lo_que_no_coincide_no_aparece() {
        assert!(buscar(&secciones(), "zzzz", "es", 10).is_empty());
    }

    #[test]
    fn un_catalogo_roto_no_tira_nada_abajo() {
        // Si el archivo instalado no se entiende, el lanzador funciona igual
        // sin esas filas.
        assert!(leer("{ esto no es json").is_err());
        assert!(leer("[]").unwrap().is_empty());
    }
}
