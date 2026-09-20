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

/// Cuánto vale encontrarla por una palabra clave y no por su nombre.
///
/// Por debajo del nombre, y a propósito: quien escribe «teclado» tiene que
/// llegar antes a «Teclado y Ratón», que se llama así, que a «Atajos», que lo
/// tiene como palabra. Una palabra clave es para que la sección **aparezca**
/// cuando no se la sabe nombrar, no para que le gane a la que sí se nombró.
///
/// Con 0,75 una coincidencia exacta con una palabra —«sonido»— vale 75, más que
/// cualquier coincidencia parcial con un nombre y menos que un nombre entero.
const PESO_DE_LA_PALABRA: f64 = 0.75;

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Seccion {
    /// Lo que se le pasa al programa: `vasak-settings network-wifi`.
    pub id: String,
    pub icono: String,
    /// El nombre en cada idioma, ya resuelto.
    pub nombres: std::collections::HashMap<String, String>,
    /// Con qué más se la puede encontrar, por idioma.
    ///
    /// Con `default` porque el catálogo puede no traerlas: las publica
    /// `vasak-settings` desde la 0.19 y sólo para las secciones donde el nombre
    /// no alcanza. Una instalación con la configuración vieja —o con una
    /// sección que no las necesita— tiene que seguir funcionando igual, con el
    /// nombre solo.
    #[serde(default)]
    pub palabras: std::collections::HashMap<String, Vec<String>>,
}

impl Seccion {
    /// Si el identificador es uno que la configuración va a entender.
    ///
    /// Se comprueba al leer y no sólo al abrir: una fila con un identificador
    /// raro se puede elegir, y recién ahí falla. Mejor que no esté.
    ///
    /// La que se descarta es **la fila**, no el catálogo entero: perder las
    /// treinta y dos secciones buenas porque una vino mal es peor que perder
    /// esa una.
    pub fn se_puede_abrir(&self) -> bool {
        !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    }

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

    /// Las palabras del idioma de la sesión.
    ///
    /// Sin caer a otro idioma, al revés que `nombre`: un nombre en inglés se
    /// puede leer y elegir, pero buscar en español con las palabras en inglés
    /// no encuentra nada y de paso ensucia el puntaje de las que sí están.
    pub fn palabras(&self, idioma: &str) -> &[String] {
        self.palabras.get(idioma).map(Vec::as_slice).unwrap_or(&[])
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
    let leidas: Vec<Seccion> = serde_json::from_str(contenido)?;
    Ok(leidas.into_iter().filter(Seccion::se_puede_abrir).collect())
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

            // El mejor de los dos caminos, no la suma: una sección que coincide
            // por el nombre **y** por una palabra no es más relevante que una
            // que coincide igual de bien por el nombre solo.
            let por_el_nombre = puntaje::puntaje(consulta, nombre) * PESO;
            let por_una_palabra = seccion
                .palabras(idioma)
                .iter()
                .map(|palabra| puntaje::puntaje(consulta, palabra) * PESO_DE_LA_PALABRA)
                .fold(0.0_f64, f64::max);
            let suyo = por_el_nombre.max(por_una_palabra);

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

    /// El mismo, con las palabras que publica la configuración desde la 0.19.
    ///
    /// Con los casos que se pisan de verdad y no con tres inventados: «Atajos»
    /// tiene «teclado» entre sus palabras y hay dos secciones que se llaman con
    /// esa palabra, que es donde se ve si el nombre le gana a la palabra.
    const CON_PALABRAS: &str = r#"[
        {"id": "multimedia-audio", "icono": "audio-speakers", "nombres": {"es": "Audio salida", "en": "Audio output"},
         "palabras": {"es": ["sonido", "volumen", "parlantes"], "en": ["sound", "volume", "speakers"]}},
        {"id": "shortcuts", "icono": "input-keyboard", "nombres": {"es": "Atajos", "en": "Shortcuts"},
         "palabras": {"es": ["atajos de teclado", "teclas"], "en": ["keybindings", "keys"]}},
        {"id": "wayfire-input", "icono": "input-mouse", "nombres": {"es": "Teclado y Ratón", "en": "Keyboard and Mouse"},
         "palabras": {"es": ["mouse", "touchpad"], "en": ["touchpad", "pointer"]}},
        {"id": "monitors", "icono": "video-display", "nombres": {"es": "Pantallas", "en": "Displays"},
         "palabras": {"es": ["resolución", "monitor"], "en": ["resolution", "scaling"]}},
        {"id": "network-bluetooth", "icono": "bluetooth", "nombres": {"es": "Bluetooth", "en": "Bluetooth"}}
    ]"#;

    fn con_palabras() -> Vec<Seccion> {
        leer(CON_PALABRAS).expect("el catálogo con palabras parsea")
    }

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
    fn una_seccion_con_un_identificador_raro_no_entra() {
        // Se puede elegir y recién ahí falla, que es peor que no ofrecerla. Y se
        // descarta la fila y no el catálogo: perder las buenas porque una vino
        // mal es peor.
        let contenido = r#"[
            {"id": "network-wifi", "icono": "x", "nombres": {"es": "Wi-Fi"}},
            {"id": "algo; rm -rf", "icono": "x", "nombres": {"es": "Raro"}},
            {"id": "", "icono": "x", "nombres": {"es": "Vacía"}},
            {"id": "Monitors", "icono": "x", "nombres": {"es": "Mayúsculas"}}
        ]"#;

        let leidas = leer(contenido).unwrap();
        assert_eq!(leidas.len(), 1);
        assert_eq!(leidas[0].id, "network-wifi");
    }

    #[test]
    fn un_catalogo_roto_no_tira_nada_abajo() {
        // Si el archivo instalado no se entiende, el lanzador funciona igual
        // sin esas filas.
        assert!(leer("{ esto no es json").is_err());
        assert!(leer("[]").unwrap().is_empty());
    }

    #[test]
    fn se_encuentra_por_una_palabra_que_no_esta_en_el_nombre() {
        // Es todo el motivo del campo: nadie escribe «Audio salida» cuando
        // quiere bajar el volumen.
        let secciones = con_palabras();
        assert_eq!(
            buscar(&secciones, "sonido", "es", 10)[0].id,
            "multimedia-audio"
        );
        assert_eq!(
            buscar(&secciones, "volumen", "es", 10)[0].id,
            "multimedia-audio"
        );
        assert_eq!(buscar(&secciones, "resolución", "es", 10)[0].id, "monitors");
        assert_eq!(buscar(&secciones, "mouse", "es", 10)[0].id, "wayfire-input");
    }

    #[test]
    fn la_fila_dice_como_se_llama_la_seccion_y_no_la_palabra() {
        // Quien escribió «sonido» tiene que ver «Audio salida», que es lo que va
        // a encontrar cuando la ventana se abra. Una fila que dijera «sonido» es
        // una promesa que la pantalla no cumple.
        let filas = buscar(&con_palabras(), "sonido", "es", 10);
        assert_eq!(filas[0].titulo, "Audio salida");
    }

    #[test]
    fn el_nombre_le_gana_a_la_palabra() {
        // «teclado» está en el nombre de una sección y entre las palabras de
        // otra. Quien lo escribe quiere la que se llama así.
        let filas = buscar(&con_palabras(), "teclado", "es", 10);
        assert_eq!(filas[0].id, "wayfire-input", "gana la que se llama así");
        assert!(
            filas.iter().any(|una| una.id == "shortcuts"),
            "pero la otra aparece igual: para eso está la palabra"
        );
        let atajos = filas.iter().position(|una| una.id == "shortcuts").unwrap();
        assert!(atajos > 0, "y aparece después");
    }

    #[test]
    fn coincidir_por_las_dos_cosas_no_suma() {
        // Si sumara, agregarle palabras a una sección la empujaría hacia arriba
        // en consultas donde no aportan nada, y el orden que hoy está bien se
        // daría vuelta al agregar sinónimos en otro lado.
        //
        // La misma sección dos veces, con palabras y sin ellas, y una consulta
        // que le calza a las dos cosas: «audio» está en el nombre y adentro de
        // una de las palabras.
        let con = leer(
            r#"[{"id": "a", "icono": "x", "nombres": {"es": "Audio salida"},
                 "palabras": {"es": ["audio de los parlantes"]}}]"#,
        )
        .unwrap();
        let sin =
            leer(r#"[{"id": "a", "icono": "x", "nombres": {"es": "Audio salida"}}]"#).unwrap();

        let con_palabras = buscar(&con, "audio", "es", 10)[0].puntaje;
        let sin_palabras = buscar(&sin, "audio", "es", 10)[0].puntaje;

        assert_eq!(
            con_palabras, sin_palabras,
            "el nombre manda y la palabra no suma"
        );
    }

    #[test]
    fn las_palabras_del_otro_idioma_no_cuentan() {
        // Buscar en español con las palabras en inglés no encuentra lo que se
        // quiere y de paso ensucia el puntaje de las que sí están.
        let secciones = con_palabras();
        let en_espanol = buscar(&secciones, "speakers", "es", 10);
        assert!(
            !en_espanol.iter().any(|una| una.id == "multimedia-audio"),
            "«speakers» es la palabra en inglés"
        );
        assert_eq!(
            buscar(&secciones, "speakers", "en", 10)[0].id,
            "multimedia-audio"
        );
    }

    #[test]
    fn una_seccion_sin_palabras_sigue_estando() {
        // La mayoría no las necesita, y el catálogo instalado hoy no las trae
        // para ninguna. Si esto se rompiera, actualizar el lanzador antes que la
        // configuración dejaría el proveedor vacío.
        let secciones = con_palabras();
        assert_eq!(
            buscar(&secciones, "bluetooth", "es", 10)[0].id,
            "network-bluetooth"
        );

        let viejo = leer(CATALOGO).unwrap();
        assert!(viejo.iter().all(|una| una.palabras("es").is_empty()));
        assert_eq!(buscar(&viejo, "pantallas", "es", 10)[0].id, "monitors");
    }

    #[test]
    fn una_palabra_vale_menos_que_el_nombre_al_que_le_calza_igual() {
        // El número, no el orden: es lo que hace que agregar palabras no pueda
        // dar vuelta un resultado que hoy está bien.
        let secciones = con_palabras();
        let por_palabra = buscar(&secciones, "sonido", "es", 10)[0].puntaje;
        let por_nombre = buscar(&secciones, "bluetooth", "es", 10)[0].puntaje;
        assert!(por_palabra < por_nombre);
    }
}
