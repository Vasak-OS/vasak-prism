//! Lo que contesta además de las aplicaciones.
//!
//! Por ahora es uno solo —el cálculo—, así que no hay ninguna abstracción de
//! proveedores y no tiene por qué haberla: con dos, un rasgo y un registro
//! serían andamiaje alrededor de un `if`. Cuando entre el tercero —los archivos
//! (#7), que además tardan y van a necesitar resultados parciales— va a hacer
//! falta de verdad, y ahí se escribe sabiendo qué forma tienen los tres.

pub mod calculo;
pub mod expresion;
pub mod unidades;
