//! Punto de entrada de una aplicación de VasakOS.
//!
//! Lo que hay acá no es decoración: cada pieza resuelve algo que en las
//! aplicaciones reales del escritorio se rompió al menos una vez.

pub mod catalogo;
pub mod comandos;
pub mod lanzador;
mod locales;

use std::sync::{Arc, Mutex};

use tauri::{Emitter, Manager};

use catalogo::{cache, escaneo, frecuencia, vigilancia, Catalogo};

/// Lo que se emite cuando el catálogo cambió y hay que volver a buscar.
const CATALOGO_CAMBIADO: &str = "catalogo-cambiado";

/// Deja el catálogo listo y lo mantiene al día.
///
/// El orden es el punto: primero la caché, que es lo que permite contestar
/// enseguida, y recién después la revalidación y la vigilancia, las dos en otro
/// hilo. Al revés, la ventana no podría buscar hasta terminar de leer el disco.
fn preparar_el_catalogo(app: &tauri::AppHandle) -> Arc<Catalogo> {
    let catalogo = Arc::new(Catalogo::nuevo(
        locales::idioma_del_sistema(),
        catalogo::escritorios_actuales(),
        cache::ruta_por_defecto(),
    ));

    catalogo.cargar_de_cache();

    let revalidar = Arc::clone(&catalogo);
    let avisar = app.clone();
    std::thread::spawn(move || {
        if !revalidar.esta_al_dia() && revalidar.reindexar() {
            let _ = avisar.emit(CATALOGO_CAMBIADO, ());
        }
    });

    let vigilado = Arc::clone(&catalogo);
    let avisar = app.clone();
    // Que no se pueda vigilar no es motivo para no tener lanzador: el índice
    // queda como está hasta el próximo arranque.
    let _ = vigilancia::vigilar(escaneo::directorios(), vigilancia::REPOSO, move || {
        if vigilado.reindexar() {
            let _ = avisar.emit(CATALOGO_CAMBIADO, ());
        }
    });

    catalogo
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // El idioma de la sesión. **Con la ruta explícita de los catálogos**:
        // el plugin sólo prueba rutas relativas al ejecutable y al directorio
        // de trabajo, y ninguna existe cuando el binario está en /usr/bin. Sin
        // esto, un paquete instalado muestra las claves crudas
        // («views.home.title») en lugar de los textos. Ver `locales.rs`.
        .plugin(tauri_plugin_i18n_vsk::init_with_path(
            Some(locales::idioma_del_sistema()),
            locales::directorio(),
        ))
        // El clic derecho abre el menú de VasakOS y no el del motor del
        // navegador, que ofrece «Recargar» e «Inspeccionar elemento».
        // El diario del sistema, con el nombre de esta aplicación. Va **primero**
        // de todos los plugins: instala el gancho de pánico, y un pánico mientras
        // arranca otro plugin es de los más probables y de los que menos rastro
        // dejan — sin esto, sólo queda un volcado de núcleo sin símbolos.
        .plugin(tauri_plugin_vsk_journal::init())
        .plugin(tauri_plugin_vsk_contextual_menu::init())
        .plugin(tauri_plugin_config_manager::init())
        .plugin(tauri_plugin_vicons::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            let manejador = app.handle().clone();
            let catalogo = preparar_el_catalogo(&manejador);

            let uso = frecuencia::ruta_por_defecto()
                .and_then(|ruta| frecuencia::Uso::abrir(&ruta).ok())
                .map(|uso| Arc::new(Mutex::new(uso)));

            app.manage(comandos::Estado {
                catalogo,
                uso,
                hay_scope: lanzador::comando::hay_scope(),
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![comandos::buscar, comandos::lanzar])
        .run(tauri::generate_context!())
        .expect("error al ejecutar la aplicación");
}
