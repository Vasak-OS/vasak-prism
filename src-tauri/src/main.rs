// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use vasak_prism_lib::argumentos::{leer, Invocacion};
use vasak_prism_lib::servicio;

fn main() {
    let argumentos: Vec<String> = std::env::args().skip(1).collect();

    match leer(&argumentos) {
        Invocacion::Version => {
            println!("vasak-prism {}", env!("CARGO_PKG_VERSION"));
        }
        // Lo que hace el atajo del teclado. Primero se pregunta si ya hay un
        // daemon: si lo hay, se le pasa el pedido y este proceso termina en
        // milisegundos sin haber levantado ningún WebView. Si no lo hay —la
        // primera vez, antes de que la sesión levante el servicio—, este proceso
        // pasa a ser el daemon y abre la ventana.
        Invocacion::Alternar => {
            if !servicio::pedir_alternar() {
                vasak_prism_lib::run(true);
            }
        }
        Invocacion::Daemon => vasak_prism_lib::run(false),
    }
}
