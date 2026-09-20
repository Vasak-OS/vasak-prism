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

    // La cuenta va primera y sin gastar nada: no toca el disco ni la red, y si
    // lo que se escribió no es una, no devuelve fila.
    let calculo = crate::proveedores::calculo::resolver(&consulta);

    let pesos = estado
        .uso
        .as_ref()
        .map(|uso| uso.lock().unwrap_or_else(|e| e.into_inner()).pesos())
        .unwrap_or_default();

    let mut filas = estado.catalogo.buscar_con_uso(&consulta, limite, &pesos);

    if let Some(cuenta) = calculo {
        // Adelante de todo y recortando el último: el límite es cuántas filas
        // se mandan, no cuántas se buscan.
        filas.insert(0, cuenta);
        filas.truncate(limite);
    }

    filas
}

/// Copia un texto al portapapeles.
///
/// Por `wl-copy` y no por una API de Wayland propia: en esta sesión el
/// portapapeles es un protocolo privilegiado y el permiso lo tiene `wl-copy`,
/// que es justo para esto. Pedirlo acá sería pedir un permiso más para hacer lo
/// mismo.
#[tauri::command]
pub fn copiar(texto: String) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};

    let mut hijo = Command::new("wl-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("no se pudo copiar: {error}"))?;

    hijo.stdin
        .as_mut()
        .ok_or("wl-copy no aceptó la entrada")?
        .write_all(texto.as_bytes())
        .map_err(|error| format!("no se pudo escribir en wl-copy: {error}"))?;

    // Se espera a que termine: `wl-copy` sin `--foreground` se va al fondo solo,
    // así que esto vuelve enseguida, y no esperarlo dejaría un zombi por cada
    // número copiado.
    let estado = hijo
        .wait()
        .map_err(|error| format!("wl-copy terminó mal: {error}"))?;

    // `wait` devuelve `Ok` para un hijo que terminó mal, así que sin mirar el
    // estado esto decía que copió cuando no copió nada — y el usuario se entera
    // recién al pegar.
    if !estado.success() {
        return Err(format!("wl-copy terminó con {estado}"));
    }

    Ok(())
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
