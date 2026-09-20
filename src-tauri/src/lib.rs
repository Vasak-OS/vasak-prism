//! Punto de entrada de una aplicación de VasakOS.
//!
//! Lo que hay acá no es decoración: cada pieza resuelve algo que en las
//! aplicaciones reales del escritorio se rompió al menos una vez.

pub mod argumentos;
pub mod catalogo;
pub mod comandos;
pub mod lanzador;
mod locales;
pub mod proveedores;
pub mod servicio;
pub mod ventana;

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

/// Arranca el daemon.
///
/// `mostrar_al_arrancar` es para cuando el atajo no encontró a nadie corriendo:
/// este proceso pasa a ser el daemon **y** abre la ventana, porque quien apretó
/// la tecla quería el lanzador y no un servicio.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run(mostrar_al_arrancar: bool) {
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
        .setup(move |app| {
            let manejador = app.handle().clone();
            let catalogo = preparar_el_catalogo(&manejador);

            let uso = frecuencia::ruta_por_defecto()
                .and_then(|ruta| frecuencia::Uso::abrir(&ruta).ok())
                .map(|uso| Arc::new(Mutex::new(uso)));

            app.manage(comandos::Estado {
                catalogo,
                uso,
                hay_scope: lanzador::comando::hay_scope(),
                recientes: Mutex::new(proveedores::recientes::Cache::nueva()),
                archivos: Mutex::new(proveedores::archivos::Indice::del_lugar_de_siempre()),
                ventanas: Mutex::new(proveedores::ventanas::Cache::nueva()),
                secciones: proveedores::configuracion::del_disco(),
                idioma: locales::idioma_del_sistema(),
            });

            // La ventana se construye acá, escondida, y no se vuelve a construir
            // nunca. Es de lo que depende que abrir el lanzador sea instantáneo.
            ventana::montar(&manejador)?;

            if mostrar_al_arrancar {
                ventana::mostrar(&manejador);
            }

            // El nombre en el bus es además la instancia única: si ya lo tiene
            // otro, este proceso sobra. No debería llegar acá —`main` pregunta
            // antes—, pero dos arranques a la vez entran los dos.
            let del_servicio = manejador.clone();
            tauri::async_runtime::spawn(async move {
                match servicio::servir(del_servicio.clone()).await {
                    Ok(conexion) => {
                        // La conexión tiene que seguir viva o el nombre se
                        // suelta y el atajo deja de encontrar a nadie.
                        std::future::pending::<()>().await;
                        drop(conexion);
                    }
                    Err(error) => {
                        eprintln!("no se pudo tomar {}: {error}", servicio::NOMBRE);
                        del_servicio.exit(0);
                    }
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            comandos::buscar,
            comandos::lanzar,
            comandos::copiar,
            comandos::abrir,
            comandos::ejecutar,
            comandos::abrir_configuracion,
            comandos::presentar_ventana,
            comandos::esconder
        ])
        .run(tauri::generate_context!())
        .expect("error al ejecutar la aplicación");
}
