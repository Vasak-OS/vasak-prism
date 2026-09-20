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
    /// El índice de archivos del gestor, abierto de sólo lectura.
    ///
    /// En `Option` y no abierto al arrancar a secas: puede no existir todavía
    /// —nadie escaneó— y aparecer después, así que se reintenta abrirlo mientras
    /// no esté. Cuando está, el handle se queda.
    pub archivos: Mutex<Option<crate::proveedores::archivos::Indice>>,
    /// Las ventanas abiertas, preguntadas al escritorio cada tanto.
    pub ventanas: Mutex<crate::proveedores::ventanas::Cache>,
    /// Las secciones de la configuración, leídas una vez al arrancar.
    ///
    /// No cambian mientras la sesión vive: las publica el paquete de
    /// `vasak-settings`, así que sólo cambian al actualizarlo, y ahí el
    /// lanzador se reinicia con la sesión siguiente.
    pub secciones: Vec<crate::proveedores::configuracion::Seccion>,
    /// El idioma de la sesión, para elegir el nombre de cada sección.
    pub idioma: String,
    /// Los archivos abiertos hace poco. La caché los relee sola cuando el
    /// archivo del escritorio cambia; el candado es porque Tauri atiende cada
    /// comando en su propio hilo.
    pub recientes: Mutex<crate::proveedores::recientes::Cache>,
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

    // Un prefijo manda: es una elección explícita de quien escribe, y mezclar
    // aplicaciones ahí abajo sería ruido.
    if let Some(filas) = crate::proveedores::por_prefijo(&consulta, limite) {
        return filas;
    }

    // La cuenta va primera y sin gastar nada: no toca el disco ni la red, y si
    // lo que se escribió no es una, no devuelve fila.
    let calculo = crate::proveedores::calculo::resolver(&consulta);

    let pesos = estado
        .uso
        .as_ref()
        .map(|uso| uso.lock().unwrap_or_else(|e| e.into_inner()).pesos())
        .unwrap_or_default();

    let mut filas = estado.catalogo.buscar_con_uso(&consulta, limite, &pesos);

    {
        let mut indice = estado.archivos.lock().unwrap_or_else(|e| e.into_inner());
        if indice.is_none() {
            *indice = crate::proveedores::archivos::Indice::del_lugar_de_siempre();
        }
        if let Some(indice) = indice.as_ref() {
            filas.extend(indice.buscar(&consulta, limite));
        }
    }

    {
        let mut cache = estado.ventanas.lock().unwrap_or_else(|e| e.into_inner());
        filas.extend(crate::proveedores::ventanas::buscar(
            cache.lista(),
            &consulta,
            limite,
        ));
    }

    filas.extend(crate::proveedores::configuracion::buscar(
        &estado.secciones,
        &consulta,
        &estado.idioma,
        limite,
    ));

    // Los recientes se leen del archivo donde el escritorio ya los anota, así
    // que no hay nada que indexar ni que vigilar. Sólo aparecen si el nombre
    // coincide: son archivos del usuario, no resultados que se ofrecen solos.
    {
        let mut cache = estado.recientes.lock().unwrap_or_else(|e| e.into_inner());
        filas.extend(crate::proveedores::recientes::buscar(
            cache.lista(),
            &consulta,
            limite,
        ));
    }

    filas.sort_by(|a, b| {
        b.puntaje
            .partial_cmp(&a.puntaje)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.titulo.cmp(&b.titulo))
    });

    if let Some(cuenta) = calculo {
        // Adelante de todo: es la respuesta exacta a lo que se escribió.
        filas.insert(0, cuenta);
    }

    // El límite es cuántas filas se mandan, no cuántas se buscan.
    filas.truncate(limite);
    filas
}

/// Trae al frente una ventana abierta.
///
/// Se lo pide al escritorio, que es el único que le habla al compositor.
#[tauri::command]
pub fn presentar_ventana(id: String) -> Result<(), String> {
    crate::proveedores::ventanas::presentar(&id)
}

/// Abre la configuración en una sección.
///
/// Con el identificador como argumento, que es lo que `vasak-settings` entiende:
/// `vasak-settings network-wifi`. Sin él abriría la portada, que para quien
/// eligió «Wi-Fi» es lo mismo que no hacer nada.
#[tauri::command]
pub fn abrir_configuracion(estado: State<'_, Estado>, seccion: String) -> Result<(), String> {
    // El identificador sale del archivo que publica la configuración, pero se
    // comprueba igual antes de meterlo en una línea de comandos: son minúsculas
    // y guiones, como los nombres del router.
    if seccion.is_empty()
        || !seccion
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err(format!("«{seccion}» no es un nombre de sección"));
    }

    let mut argumentos = vec!["vasak-settings".to_string(), seccion];

    if estado.hay_scope {
        argumentos = comando::envolver_en_scope(argumentos);
    }

    comando::lanzar(&argumentos)
}

/// Abre una dirección o un archivo con lo que corresponda.
///
/// Por `xdg-open`, que es quien sabe cuál es el navegador y con qué se abre un
/// `.odt`. Y en un ámbito de systemd por lo mismo que las aplicaciones: lo que
/// se abra no tiene que morirse con el lanzador.
#[tauri::command]
pub fn abrir(estado: State<'_, Estado>, destino: String) -> Result<(), String> {
    let mut argumentos = vec!["xdg-open".to_string(), destino];

    if estado.hay_scope {
        argumentos = comando::envolver_en_scope(argumentos);
    }

    comando::lanzar(&argumentos)
}

/// Ejecuta un comando de shell.
///
/// Por `sh -c` y no desarmándolo acá: quien escribe `> ls | wc -l` espera que
/// las tuberías y las comillas signifiquen lo que significan en una shell.
#[tauri::command]
pub fn ejecutar(estado: State<'_, Estado>, comando_escrito: String) -> Result<(), String> {
    let comando_escrito = comando_escrito.trim();
    if comando_escrito.is_empty() {
        return Err("no hay nada que ejecutar".to_string());
    }

    let mut argumentos = vec![
        "sh".to_string(),
        "-c".to_string(),
        comando_escrito.to_string(),
    ];

    if estado.hay_scope {
        argumentos = comando::envolver_en_scope(argumentos);
    }

    comando::lanzar(&argumentos)
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
