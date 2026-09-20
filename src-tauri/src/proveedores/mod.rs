//! Lo que contesta además de las aplicaciones.
//!
//! # Cómo se reparte la consulta
//!
//! Un **prefijo manda**: si lo escrito arranca con `>`, `?` o `:`, contesta ese
//! proveedor y nadie más. Es una elección explícita de quien escribe, y mezclar
//! aplicaciones ahí abajo sería ruido — además de que `:` y `>` aparecen
//! adentro de texto normal todo el tiempo, y sin la regla del prefijo habría
//! que adivinar.
//!
//! Sin prefijo contestan los tres que se pueden decidir mirando lo escrito: la
//! cuenta, las aplicaciones y los recientes. La cuenta no dispara si lo escrito
//! no es una, y los recientes sólo si el nombre coincide.
//!
//! # Por qué no hay un rasgo `Proveedor`
//!
//! Porque cada uno recibe algo distinto —las aplicaciones necesitan el catálogo
//! y los pesos de uso, los recientes su lista, el resto nada— y devolver todo
//! por la misma puerta obligaría a pasarles a todos lo que necesita el que más
//! necesita. Con cinco funciones y un `match` se lee de arriba abajo. El rasgo
//! va a tener sentido cuando entren los archivos (#7), que tardan y van a
//! necesitar contestar por partes.

pub mod archivos;
pub mod calculo;
pub mod configuracion;
pub mod emoji;
pub mod expresion;
pub mod recientes;
pub mod shell;
pub mod unidades;
pub mod ventanas;
pub mod web;

use crate::catalogo::aplicacion::Resultado;

/// El piso de los resultados con prefijo.
///
/// Arriba de cualquier puntaje de texto —que llega hasta 100— porque cuando hay
/// prefijo no compiten con nada: es lo único que se muestra, y el número sólo
/// ordena entre ellos.
const PISO: f64 = 1000.0;

pub fn puntaje_de_prefijo(base: f64) -> f64 {
    PISO + base
}

/// Los prefijos que se quedan con la consulta entera.
pub const PREFIJOS: &[char] = &['>', '?', ':'];

/// Contesta si lo escrito arranca con un prefijo.
///
/// `None` significa «esto no es mío»: la consulta sigue su camino normal.
pub fn por_prefijo(consulta: &str, limite: usize) -> Option<Vec<Resultado>> {
    let consulta = consulta.trim_start();
    let mut caracteres = consulta.chars();
    let prefijo = caracteres.next()?;
    let resto = caracteres.as_str();

    match prefijo {
        '>' => Some(shell::buscar(resto)),
        '?' => Some(web::buscar(resto)),
        ':' => Some(emoji::buscar(resto, limite)),
        _ => None,
    }
}
