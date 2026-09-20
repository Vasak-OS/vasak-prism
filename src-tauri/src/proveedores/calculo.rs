//! El resultado de una cuenta, como una fila más de la lista.
//!
//! Escribís `2+2` o `3 pulgadas a cm` y la respuesta está arriba de todo, con
//! Enter copiándola al portapapeles. Es de las cosas que más se usan de un
//! lanzador y de las que menos cuestan: no toca el disco ni la red.

use crate::catalogo::aplicacion::{Origen, Resultado};

use super::{expresion, unidades};

/// Una cuenta es una respuesta exacta a lo que se escribió, así que va primera.
/// Cien es el techo de los puntajes de texto; esto queda por encima.
const PUNTAJE: f64 = 1000.0;

/// El icono, por nombre del tema como todos los demás.
const ICONO: &str = "accessories-calculator";

/// Con esto adelante, lo que sigue es una cuenta y punto.
const FORZAR: char = '=';

/// La respuesta a la consulta, si es una cuenta o una conversión.
pub fn resolver(consulta: &str) -> Option<Resultado> {
    let consulta = consulta.trim();
    if consulta.is_empty() {
        return None;
    }

    // El `=` adelante pide una cuenta explícitamente: se saca antes de mirar
    // nada, y de paso habilita lo que sin él no cuenta —un número solo—. Se
    // saca acá y no en la interfaz: la sintaxis de un proveedor es del
    // proveedor, y la ventana no tiene por qué conocerla.
    let (consulta, forzada) = match consulta.strip_prefix(FORZAR) {
        Some(resto) => (resto.trim(), true),
        None => (consulta, false),
    };

    if consulta.is_empty() {
        return None;
    }

    // Primero la conversión: «2 m a cm» también parsea como una resta si se la
    // mira sin las unidades, y la conversión es lo que se quiso decir.
    let (titulo, subtitulo) = if let Some(conversion) = unidades::convertir(consulta) {
        (
            format!(
                "{} {}",
                expresion::formatear(conversion.valor),
                conversion.unidad
            ),
            expresion::formatear(conversion.valor),
        )
    } else {
        let valor = if forzada {
            expresion::evaluar_explicito(consulta)?
        } else {
            expresion::evaluar(consulta)?
        };
        let texto = expresion::formatear(valor);
        (texto.clone(), texto)
    };

    Some(Resultado {
        // Lo que se copia al elegirlo. El número solo, sin la unidad: es lo que
        // se pega en un campo o en un archivo.
        id: subtitulo,
        accion: None,
        titulo,
        // La clave del catálogo de idioma; la traduce la interfaz, que es la
        // que sabe en qué idioma está la sesión.
        subtitulo: Some("lanzador.copiar".to_string()),
        subtitulo_dato: None,
        icono: Some(ICONO.to_string()),
        puntaje: PUNTAJE,
        origen: Origen::Calculo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_cuenta_da_su_resultado() {
        let fila = resolver("2+2").expect("2+2 es una cuenta");
        assert_eq!(fila.titulo, "4");
        assert_eq!(fila.id, "4");
        assert_eq!(fila.origen, Origen::Calculo);
    }

    #[test]
    fn una_conversion_muestra_la_unidad_y_copia_el_numero() {
        // Arriba se lee «7.62 cm», que es la respuesta; lo que se copia es
        // `7.62`, que es lo que se pega en un campo.
        let fila = resolver("3 pulgadas a cm").expect("es una conversión");
        assert_eq!(fila.titulo, "7.62 cm");
        assert_eq!(fila.id, "7.62");
    }

    #[test]
    fn la_conversion_le_gana_a_la_lectura_como_cuenta() {
        // «2 m a cm» sin las unidades parece una resta. La conversión es lo que
        // se quiso decir.
        assert_eq!(resolver("2 m a cm").unwrap().titulo, "200 cm");
    }

    #[test]
    fn va_arriba_de_cualquier_aplicacion() {
        // Cien es el techo de los puntajes de texto: una cuenta es la respuesta
        // exacta a lo que se escribió, no una coincidencia.
        assert!(resolver("2+2").unwrap().puntaje > 100.0);
    }

    #[test]
    fn lo_que_no_es_una_cuenta_no_da_fila() {
        assert!(resolver("firefox").is_none());
        assert!(resolver("editor de texto").is_none());
        assert!(resolver("").is_none());
        assert!(resolver("   ").is_none());
    }

    #[test]
    fn un_numero_solo_tampoco() {
        // Escribir «42» busca lo que se llame 42.
        assert!(resolver("42").is_none());
    }

    #[test]
    fn el_igual_adelante_fuerza_la_cuenta() {
        // Y hace que un número solo sí conteste: quien escribió `=42` pidió una
        // cuenta, no una aplicación llamada 42.
        assert_eq!(resolver("=2+2").unwrap().titulo, "4");
        assert_eq!(resolver("=42").unwrap().titulo, "42");
        assert_eq!(resolver("= 2 + 2").unwrap().titulo, "4");
    }

    #[test]
    fn el_igual_solo_no_da_nada() {
        assert!(resolver("=").is_none());
        assert!(resolver("=   ").is_none());
    }

    #[test]
    fn con_el_igual_tambien_se_convierte() {
        assert_eq!(resolver("=3 pulgadas a cm").unwrap().titulo, "7.62 cm");
    }
}
