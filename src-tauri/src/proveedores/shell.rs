//! Correr un comando sin abrir una terminal para eso.
//!
//! Con `>` adelante. Es la parte más filosa del lanzador: lo que se escriba se
//! ejecuta, así que la fila lo dice con todas las letras y no se parece a una
//! aplicación. Quien escribe `> rm -rf algo` tiene que ver que va a ejecutar un
//! comando, no un resultado más de la lista.

use crate::catalogo::aplicacion::{Origen, Resultado};

const ICONO: &str = "utilities-terminal";

/// La fila para ejecutar lo escrito.
pub fn buscar(consulta: &str) -> Vec<Resultado> {
    let comando = consulta.trim();

    if comando.is_empty() {
        return Vec::new();
    }

    vec![Resultado {
        // El comando tal cual: es lo que se va a ejecutar.
        id: comando.to_string(),
        accion: None,
        titulo: comando.to_string(),
        subtitulo: Some("lanzador.ejecutar".to_string()),
        subtitulo_dato: None,
        icono: Some(ICONO.to_string()),
        puntaje: super::puntaje_de_prefijo(100.0),
        origen: Origen::Comando,
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lo_escrito_es_el_comando() {
        let filas = buscar("systemctl --user status");
        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].id, "systemctl --user status");
        assert_eq!(filas[0].origen, Origen::Comando);
    }

    #[test]
    fn la_fila_dice_que_va_a_ejecutar() {
        // Es la parte más filosa del lanzador: tiene que verse que no es una
        // aplicación más de la lista.
        assert_eq!(
            buscar("ls").pop().unwrap().subtitulo.as_deref(),
            Some("lanzador.ejecutar")
        );
    }

    #[test]
    fn sin_comando_no_hay_fila() {
        assert!(buscar("").is_empty());
        assert!(buscar("   ").is_empty());
    }

    #[test]
    fn los_espacios_de_los_costados_no_van() {
        assert_eq!(buscar("  htop  ").pop().unwrap().id, "htop");
    }
}
