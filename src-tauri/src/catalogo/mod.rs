//! El catálogo de aplicaciones: leer las entradas del escritorio y puntuarlas.
//!
//! Nada de acá toca el disco salvo `Entrada::puede_ejecutarse`, que busca el
//! `TryExec` en el `PATH`. El escaneo de directorios, la caché y la vigilancia
//! con inotify van aparte: esto es lo que se puede probar entero sin montar un
//! sistema de archivos de mentira.

pub mod entrada;
pub mod exec;
pub mod puntaje;
