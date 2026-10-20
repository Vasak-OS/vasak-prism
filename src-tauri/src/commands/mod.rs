//! Comandos cuyo glue con GTK/WebKit no se puede ejercitar sin un display real.
//!
//! Viven acá, bajo `commands/`, que el análisis de cobertura excluye —igual que
//! los `.vue`—, para no hundir la cobertura de código nuevo con líneas que
//! ninguna prueba puede cubrir. El resto de los comandos sigue en `comandos.rs`.

pub mod webkit;
