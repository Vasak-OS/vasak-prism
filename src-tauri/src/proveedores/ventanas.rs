//! Las ventanas abiertas, como resultados.
//!
//! Si Firefox ya está abierto, escribir «firefox» tiene que ofrecer **esa
//! ventana** antes que abrir otro. Es la diferencia entre volver a lo que
//! estabas haciendo y empezar de nuevo.
//!
//! La lista sale de `vasak-desktop` por D-Bus y no del compositor. En Wayland,
//! enumerar ventanas es un protocolo privilegiado —`foreign_toplevel`— y el
//! plugin `permisos-globales` lo reparte por programa: pedirlo para el lanzador
//! sería pedir también los títulos de todo lo que hay abierto, cuando el
//! escritorio ya los tiene y puede contestarlos.

use std::time::{Duration, Instant};

use serde::Deserialize;

use crate::catalogo::aplicacion::{Origen, Resultado};
use crate::catalogo::puntaje;

const SERVICIO: &str = "org.vasak.os.Desktop";
const RUTA: &str = "/org/vasak/os/Desktop";

/// Cuánto vale una ventana abierta frente a la aplicación que la abriría.
///
/// Más que 1: si Firefox está abierto, la ventana va **antes** que el Firefox
/// del menú. Volver a lo que estabas es lo que se quiso pedir casi siempre.
const PESO: f64 = 1.1;

/// Cuánto dura la lista antes de volver a preguntar.
///
/// Preguntar en cada tecla es un viaje por D-Bus por letra, y lo que se abre y
/// se cierra no cambia en medio de una palabra. Dos segundos es más de lo que
/// tarda alguien en escribir y menos de lo que se nota.
const VIGENCIA: Duration = Duration::from_secs(2);

/// Una ventana, tal cual la manda el escritorio.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct Ventana {
    /// El identificador de la vista en el compositor.
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub icon: String,
    #[serde(default)]
    pub is_minimized: bool,
}

pub fn leer(json: &str) -> Result<Vec<Ventana>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Le pregunta al escritorio qué ventanas hay.
///
/// Vacío si no contesta, que es un caso normal: el escritorio puede no estar
/// corriendo —una sesión donde el lanzador se abrió solo— y eso no es motivo
/// para no buscar en todo lo demás.
fn preguntar() -> Vec<Ventana> {
    let Ok(conexion) = zbus::blocking::Connection::session() else {
        return Vec::new();
    };

    let respuesta = conexion.call_method(Some(SERVICIO), RUTA, Some(SERVICIO), "ListWindows", &());

    let Ok(mensaje) = respuesta else {
        return Vec::new();
    };

    mensaje
        .body()
        .deserialize::<String>()
        .ok()
        .and_then(|json| leer(&json).ok())
        .unwrap_or_default()
}

/// La lista, sin preguntar más de lo necesario.
pub struct Cache {
    ventanas: Vec<Ventana>,
    preguntado: Option<Instant>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::nueva()
    }
}

impl Cache {
    pub fn nueva() -> Self {
        Self {
            ventanas: Vec::new(),
            preguntado: None,
        }
    }

    pub fn lista(&mut self) -> &[Ventana] {
        let vencida = self
            .preguntado
            .map(|cuando| cuando.elapsed() > VIGENCIA)
            .unwrap_or(true);

        if vencida {
            self.ventanas = preguntar();
            self.preguntado = Some(Instant::now());
        }

        &self.ventanas
    }
}

/// Las ventanas cuyo título coincide con lo escrito.
pub fn buscar(ventanas: &[Ventana], consulta: &str, limite: usize) -> Vec<Resultado> {
    let consulta = consulta.trim();
    if consulta.is_empty() {
        return Vec::new();
    }

    let mut filas: Vec<Resultado> = ventanas
        .iter()
        .filter_map(|ventana| {
            let suyo = puntaje::puntaje(consulta, &ventana.title) * PESO;

            (suyo > 0.0).then(|| Resultado {
                id: ventana.id.clone(),
                accion: None,
                titulo: ventana.title.clone(),
                subtitulo: Some("lanzador.ventanaAbierta".to_string()),
                subtitulo_dato: None,
                // El icono que el escritorio ya resolvió para su panel: es el
                // mismo nombre del tema que usa todo lo demás.
                icono: (!ventana.icon.is_empty()).then(|| ventana.icon.clone()),
                puntaje: suyo,
                origen: Origen::Ventana,
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

/// Le pide al escritorio que la traiga al frente.
///
/// Por él y no por el compositor, por lo mismo que la lista. Y con
/// `PresentWindow` y no con el alternar del panel: elegir una ventana en una
/// lista de resultados no puede esconderla.
pub fn presentar(id: &str) -> Result<(), String> {
    let conexion = zbus::blocking::Connection::session()
        .map_err(|error| format!("no hay bus de sesión: {error}"))?;

    conexion
        .call_method(Some(SERVICIO), RUTA, Some(SERVICIO), "PresentWindow", &id)
        .map(|_| ())
        .map_err(|error| format!("el escritorio no pudo traerla al frente: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LISTA: &str = r#"[
        {"id": "12", "title": "Firefox — Wikipedia", "icon": "firefox", "is_minimized": false},
        {"id": "34", "title": "notas.md — Editor de texto", "icon": "accessories-text-editor", "is_minimized": true}
    ]"#;

    fn ventanas() -> Vec<Ventana> {
        leer(LISTA).expect("la lista de prueba parsea")
    }

    #[test]
    fn se_lee_lo_que_manda_el_escritorio() {
        let leidas = ventanas();
        assert_eq!(leidas.len(), 2);
        assert_eq!(leidas[0].id, "12");
        assert_eq!(leidas[0].title, "Firefox — Wikipedia");
        assert!(leidas[1].is_minimized);
    }

    #[test]
    fn una_lista_que_no_se_entiende_no_tira_nada_abajo() {
        assert!(leer("no es json").is_err());
        assert!(leer("[]").unwrap().is_empty());
    }

    #[test]
    fn faltarle_campos_no_la_descarta() {
        // El escritorio puede agregar o sacar campos; lo que hace falta acá es
        // el identificador y el título.
        let minima = r#"[{"id": "7", "title": "Algo"}]"#;
        let leidas = leer(minima).unwrap();
        assert_eq!(leidas[0].icon, "");
        assert!(!leidas[0].is_minimized);
    }

    #[test]
    fn se_busca_por_el_titulo() {
        let filas = buscar(&ventanas(), "wikipedia", 10);
        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].id, "12");
        assert_eq!(filas[0].origen, Origen::Ventana);
    }

    #[test]
    fn la_ventana_abierta_le_gana_a_la_aplicacion_cerrada() {
        // Si Firefox ya está abierto, «firefox» tiene que ofrecer esa ventana
        // antes que abrir otro: volver a lo que estabas es lo que se pidió.
        let de_la_ventana = buscar(&ventanas(), "firefox", 10)[0].puntaje;
        // Lo que sacaría la aplicación con el mismo texto: empieza con la
        // consulta, que son 90.
        assert!(de_la_ventana > 90.0);
    }

    #[test]
    fn con_el_campo_vacio_no_aparecen() {
        assert!(buscar(&ventanas(), "", 10).is_empty());
    }

    #[test]
    fn una_ventana_sin_icono_no_inventa_uno() {
        let sin_icono = vec![Ventana {
            id: "1".to_string(),
            title: "Algo".to_string(),
            icon: String::new(),
            is_minimized: false,
        }];
        assert_eq!(buscar(&sin_icono, "algo", 10)[0].icono, None);
    }
}
