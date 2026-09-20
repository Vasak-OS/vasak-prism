//! Lo que el índice guarda de cada aplicación, y cómo se puntúa una consulta.

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::entrada::{Accion, Entrada};
use super::puntaje;

/// Cuánto pesa cada campo al puntuar.
///
/// El nombre manda. El resto acompaña: que «navegador» encuentre a Firefox está
/// bien, que le gane a una aplicación que se llama Navegador, no.
const PESO_GENERICO: f64 = 0.90;
const PESO_PALABRAS: f64 = 0.85;
const PESO_COMENTARIO: f64 = 0.60;
/// Una acción vale un poco menos que su aplicación, para que «firefox» ofrezca
/// primero Firefox y después sus ventanas.
const PESO_ACCION: f64 = 0.95;

/// Una aplicación del índice.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aplicacion {
    pub id: String,
    pub nombre: String,
    pub generico: Option<String>,
    pub comentario: Option<String>,
    pub palabras: Vec<String>,
    pub icono: Option<String>,
    pub exec: String,
    pub terminal: bool,
    pub acciones: Vec<Accion>,
    pub ruta: String,
    /// Cuándo se modificó el archivo, para saber si la caché quedó vieja.
    pub mtime: i64,
}

impl Aplicacion {
    pub fn desde(entrada: Entrada, ruta: &Path, mtime: i64) -> Self {
        Self {
            id: entrada.id,
            nombre: entrada.nombre,
            generico: entrada.nombre_generico,
            comentario: entrada.comentario,
            palabras: entrada.palabras_clave,
            icono: entrada.icono,
            exec: entrada.exec,
            terminal: entrada.terminal,
            acciones: entrada.acciones,
            ruta: ruta.to_string_lossy().to_string(),
            mtime,
        }
    }

    /// Cuánto se parece la consulta a esta aplicación, de 0 a 100.
    pub fn puntaje(&self, consulta: &str) -> f64 {
        let mut mejor = puntaje::puntaje(consulta, &self.nombre);

        if let Some(generico) = &self.generico {
            mejor = mejor.max(puntaje::puntaje(consulta, generico) * PESO_GENERICO);
        }

        for palabra in &self.palabras {
            mejor = mejor.max(puntaje::puntaje(consulta, palabra) * PESO_PALABRAS);
        }

        if let Some(comentario) = &self.comentario {
            mejor = mejor.max(puntaje::puntaje(consulta, comentario) * PESO_COMENTARIO);
        }

        mejor
    }
}

/// Una fila de la lista de resultados.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resultado {
    /// El identificador de la aplicación.
    pub id: String,
    /// La acción, si esta fila es una acción y no la aplicación.
    pub accion: Option<String>,
    pub titulo: String,
    pub subtitulo: Option<String>,
    pub icono: Option<String>,
    pub puntaje: f64,
}

/// Los resultados que esta aplicación aporta para una consulta: ella y las
/// acciones que coincidan.
///
/// Las acciones son resultados propios y no un menú adentro del resultado: «Abrir
/// una ventana privada» tiene que poder elegirse con Enter, no con un segundo
/// paso.
pub fn resultados(aplicacion: &Aplicacion, consulta: &str) -> Vec<Resultado> {
    let mut salida = Vec::new();

    let propio = aplicacion.puntaje(consulta);
    if propio > 0.0 {
        salida.push(Resultado {
            id: aplicacion.id.clone(),
            accion: None,
            titulo: aplicacion.nombre.clone(),
            subtitulo: aplicacion
                .comentario
                .clone()
                .or_else(|| aplicacion.generico.clone()),
            icono: aplicacion.icono.clone(),
            puntaje: propio,
        });
    }

    for accion in &aplicacion.acciones {
        let suyo = puntaje::puntaje(consulta, &accion.nombre) * PESO_ACCION;
        // La acción también aparece si la coincidencia es con la aplicación:
        // quien escribe «firefox» quiere ver la ventana privada entre las
        // opciones, no sólo si escribe «privada».
        let puntaje_final = suyo.max(propio * PESO_ACCION);

        if puntaje_final > 0.0 {
            salida.push(Resultado {
                id: aplicacion.id.clone(),
                accion: Some(accion.id.clone()),
                titulo: accion.nombre.clone(),
                subtitulo: Some(aplicacion.nombre.clone()),
                icono: accion.icono.clone().or_else(|| aplicacion.icono.clone()),
                puntaje: puntaje_final,
            });
        }
    }

    salida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn una(nombre: &str) -> Aplicacion {
        Aplicacion {
            id: format!("{nombre}.desktop"),
            nombre: nombre.to_string(),
            generico: None,
            comentario: None,
            palabras: Vec::new(),
            icono: None,
            exec: nombre.to_string(),
            terminal: false,
            acciones: Vec::new(),
            ruta: format!("/usr/share/applications/{nombre}.desktop"),
            mtime: 0,
        }
    }

    #[test]
    fn el_nombre_le_gana_a_la_descripcion() {
        // Que «navegador» encuentre a Firefox está bien; que le gane a una
        // aplicación llamada Navegador, no.
        let mut firefox = una("Firefox");
        firefox.comentario = Some("Navegador web".to_string());
        let navegador = una("Navegador");

        assert!(navegador.puntaje("navegador") > firefox.puntaje("navegador"));
    }

    #[test]
    fn las_palabras_clave_encuentran_lo_que_el_nombre_no_dice() {
        let mut terminal = una("Konsole");
        terminal.palabras = vec!["terminal".to_string(), "shell".to_string()];

        assert!(terminal.puntaje("terminal") > 0.0);
        assert!(terminal.puntaje("shell") > 0.0);
    }

    #[test]
    fn lo_que_no_coincide_con_nada_no_da_resultado() {
        assert!(resultados(&una("Firefox"), "zzz").is_empty());
    }

    #[test]
    fn las_acciones_son_filas_propias() {
        let mut firefox = una("Firefox");
        firefox.acciones = vec![Accion {
            id: "privada".to_string(),
            nombre: "Nueva ventana privada".to_string(),
            exec: "firefox --private-window".to_string(),
            icono: None,
        }];

        let filas = resultados(&firefox, "firefox");

        assert_eq!(filas.len(), 2);
        assert_eq!(filas[0].accion, None);
        assert_eq!(filas[1].accion.as_deref(), Some("privada"));
        // Y la aplicación va primero.
        assert!(filas[0].puntaje > filas[1].puntaje);
    }

    #[test]
    fn una_accion_se_encuentra_por_su_propio_nombre() {
        let mut firefox = una("Firefox");
        firefox.acciones = vec![Accion {
            id: "privada".to_string(),
            nombre: "Nueva ventana privada".to_string(),
            exec: "firefox --private-window".to_string(),
            icono: None,
        }];

        let filas = resultados(&firefox, "privada");

        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].accion.as_deref(), Some("privada"));
    }

    #[test]
    fn la_accion_hereda_el_icono_de_la_aplicacion() {
        let mut firefox = una("Firefox");
        firefox.icono = Some("firefox".to_string());
        firefox.acciones = vec![Accion {
            id: "privada".to_string(),
            nombre: "Ventana privada".to_string(),
            exec: "firefox --private-window".to_string(),
            icono: None,
        }];

        let filas = resultados(&firefox, "firefox");
        assert_eq!(filas[1].icono.as_deref(), Some("firefox"));
    }
}
