//! Lo que la ventana le puede pedir al backend.
//!
//! Son dos: buscar y lanzar. Todo el trabajo —leer el disco, puntuar, ordenar,
//! armar el comando— pasa de este lado; la ventana pinta lo que llega y no
//! decide nada. Es lo que permite que escribir una letra cueste una llamada y no
//! quinientas.

use std::sync::{Arc, Mutex};

use tauri::State;

use crate::catalogo::aplicacion::Resultado;
use crate::catalogo::{frecuencia, Catalogo};
use crate::lanzador::comando;

/// Cuántos resultados se devuelven como mucho.
///
/// La lista muestra ocho a la vez; el resto es para desplazarse. Mandar
/// quinientos sería serializar quinientos para dibujar ocho.
const LIMITE: usize = 50;

/// Lo que el backend tiene puesto mientras la aplicación vive.
pub struct Estado {
    pub catalogo: Arc<Catalogo>,
    /// El registro de uso, si se pudo abrir. Que no se pueda no es motivo para
    /// quedarse sin lanzador: se pierde el aprendizaje, nada más.
    ///
    /// Bajo candado porque una conexión de SQLite no se puede compartir entre
    /// hilos, y Tauri atiende cada comando en el suyo. No estorba: se toca al
    /// buscar y al lanzar, nunca en medio de nada largo.
    pub uso: Option<Arc<Mutex<frecuencia::Uso>>>,
    /// Si en esta máquina hay `systemd-run`. Se mira una vez, al arrancar, y no
    /// en cada lanzamiento.
    pub hay_scope: bool,
}

/// Esconde la ventana.
///
/// Lo llama la interfaz al elegir un resultado. El Escape y la pérdida de foco
/// no pasan por acá: los atiende la superficie de capa, que es la dueña del
/// teclado y la única que se entera de un clic que cayó en otra ventana.
#[tauri::command]
pub fn esconder(app: tauri::AppHandle) {
    crate::ventana::esconder(&app);
}

#[tauri::command]
pub fn buscar(
    estado: State<'_, Estado>,
    consulta: String,
    limite: Option<usize>,
) -> Vec<Resultado> {
    let limite = limite.unwrap_or(LIMITE).min(LIMITE);

    let pesos = estado
        .uso
        .as_ref()
        .map(|uso| uso.lock().unwrap_or_else(|e| e.into_inner()).pesos())
        .unwrap_or_default();

    estado.catalogo.buscar_con_uso(&consulta, limite, &pesos)
}

#[tauri::command]
pub fn lanzar(estado: State<'_, Estado>, id: String, accion: Option<String>) -> Result<(), String> {
    let aplicaciones = estado.catalogo.aplicaciones();
    let aplicacion = aplicaciones
        .iter()
        .find(|app| app.id == id)
        .ok_or_else(|| format!("no está la aplicación {id}"))?;

    // Una acción trae su propia línea; sin acción, la de la aplicación.
    let linea = match &accion {
        Some(cual) => {
            &aplicacion
                .acciones
                .iter()
                .find(|una| &una.id == cual)
                .ok_or_else(|| format!("{id} no tiene la acción {cual}"))?
                .exec
        }
        None => &aplicacion.exec,
    };

    let argumentos = comando::armar(
        linea,
        &aplicacion.nombre,
        aplicacion.icono.as_deref(),
        &aplicacion.ruta,
        aplicacion.terminal,
        estado.hay_scope,
    )
    .ok_or_else(|| format!("no se pudo armar el comando de {id}"))?;

    comando::lanzar(&argumentos)?;

    // Se anota después de lanzar y no antes: lo que no se pudo abrir no cuenta
    // como usado, o el primer puesto se lo quedaría una entrada rota.
    if let Some(uso) = &estado.uso {
        let _ = uso
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .anotar(&frecuencia::clave(&id, accion.as_deref()));
    }

    Ok(())
}
