//! La ventana, que se construye una vez y se muestra muchas.
//!
//! Es de lo que depende que el lanzador se sienta instantáneo. La búsqueda que
//! había en el escritorio creaba la ventana en cada apertura y la **cerraba** al
//! perder el foco, así que cada vez pagaba el arranque completo de un WebView:
//! unos cientos de milisegundos con la pantalla vacía. Acá la ventana se
//! construye al levantar la sesión, escondida, y el atajo sólo la muestra.
//!
//! # Por qué una superficie de capa y no una ventana normal
//!
//! En Wayland el compositor pone las ventanas donde quiere. Un lanzador tiene
//! que aparecer centrado y encima de todo, y tiene que poder quedarse con el
//! teclado aunque no sea la ventana «activa»; nada de eso se pide con una
//! ventana común. `gtk-layer-shell` sí, así que el WebView de Tauri se muda a
//! una ventana de capa y el armazón que Tauri creó queda escondido y vacío.
//!
//! # Y qué pasa si el compositor no la da
//!
//! `zwlr_layer_shell_v1` es un protocolo privilegiado: en VasakOS lo reparte el
//! plugin `permisos-globales` de wayfire, y sólo a los programas que lo
//! necesitan. Hasta que Prism esté en esa lista —y en cualquier otro
//! escritorio— el protocolo no está.
//!
//! Sin comprobarlo, eso no era un modo degradado sino un **aborto**:
//! gtk-layer-shell avisa que cae a XDG shell, las llamadas siguientes se hacen
//! igual sobre una superficie que no es de capa, y libwayland corta el proceso
//! con SIGABRT. Un lanzador que no arranca es peor que uno mal ubicado, así que
//! se pregunta antes y, si no está, se usa la ventana común: se ve peor —el
//! compositor la pone donde quiere y no se queda con el teclado— pero abre.
//!
//! Los objetos de GTK no son `Send`, así que la ventana vive en un
//! `thread_local` del hilo principal y todo lo que la toca pasa por
//! `run_on_main_thread`.

use std::cell::RefCell;

use gtk::prelude::*;
use gtk_layer_shell::{KeyboardMode, Layer, LayerShell};
use tauri::{AppHandle, Emitter, WebviewUrl, WebviewWindowBuilder};

/// El nombre de la ventana de Tauri. Es el que nombra la capacidad.
pub const ETIQUETA: &str = "main";

/// Cómo se identifica la superficie ante el compositor.
const ESPACIO: &str = "vasak-prism";

const ANCHO: f64 = 720.0;
const ALTO: f64 = 560.0;

/// Se emite cuando la ventana aparece: la interfaz enfoca el campo y limpia lo
/// que hubiera quedado de la vez anterior.
pub const MOSTRADA: &str = "prism:mostrada";

/// La ventana que se muestra, sea cual sea la que se pudo armar.
enum Superficie {
    /// Lo normal en VasakOS: centrada, encima de todo y con el teclado.
    Capa(gtk::Window),
    /// Lo que queda cuando el compositor no da layer-shell. Se guarda la ventana
    /// de Tauri y no la de GTK: mostrarla a mano con `gtk::Window::show` no
    /// alcanza —los widgets de adentro nunca se mostraron y la ventana aparece
    /// vacía o no aparece—, y Tauri ya sabe hacerlo.
    /// En caja porque una `WebviewWindow` pesa casi un kilobyte y la otra
    /// variante ocho bytes: sin la caja, el enum entero mide lo que la más
    /// grande y se paga en cada copia.
    Comun(Box<tauri::WebviewWindow>),
}

impl Superficie {
    fn mostrar(&self) {
        match self {
            Self::Capa(ventana) => {
                ventana.show();
                ventana.present();
            }
            Self::Comun(ventana) => {
                let _ = ventana.show();
                let _ = ventana.set_focus();
            }
        }
    }

    fn esconder(&self) {
        match self {
            Self::Capa(ventana) => ventana.hide(),
            Self::Comun(ventana) => {
                let _ = ventana.hide();
            }
        }
    }

    fn esta_visible(&self) -> bool {
        match self {
            Self::Capa(ventana) => ventana.is_visible(),
            Self::Comun(ventana) => ventana.is_visible().unwrap_or(false),
        }
    }
}

thread_local! {
    /// La ventana. Se guarda y no se olvida con `mem::forget` porque hay que
    /// poder mostrarla y esconderla, que es todo lo que hace.
    static VENTANA: RefCell<Option<Superficie>> = const { RefCell::new(None) };
}

/// Construye la ventana, escondida. Se llama una sola vez, al arrancar.
pub fn montar(app: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let webview = WebviewWindowBuilder::new(app, ETIQUETA, WebviewUrl::App("index.html".into()))
        .title("Prism")
        .decorations(false)
        .transparent(true)
        .inner_size(ANCHO, ALTO)
        .visible(false)
        .skip_taskbar(true)
        .build()?;

    let armazon = webview.gtk_window()?;

    // Se pregunta antes de tocar nada: gtk-layer-shell avisa y sigue igual, y
    // la llamada siguiente sobre una superficie que no es de capa termina en un
    // aborto de libwayland.
    if !gtk_layer_shell::is_supported() {
        eprintln!(
            "vasak-prism: el compositor no da zwlr_layer_shell_v1; \
             la ventana va a ser una común, sin centrado ni teclado exclusivo"
        );
        montar_comun(&armazon, &webview);
        VENTANA
            .with(|guardada| *guardada.borrow_mut() = Some(Superficie::Comun(Box::new(webview))));
        return Ok(());
    }

    let capa = gtk::Window::new(gtk::WindowType::Toplevel);
    capa.set_decorated(false);
    // La capa pide el tamaño a esta ventana y no al armazón de Tauri: sin
    // pedirlo, el eje que no está anclado a sus dos bordes se encoge a lo que
    // pida WebKit, que es nada.
    capa.set_size_request(ANCHO as i32, ALTO as i32);

    capa.init_layer_shell();
    capa.set_namespace(ESPACIO);
    // Encima de todo, incluido el panel: si el lanzador apareciera debajo,
    // buscar con el panel en pantalla completa no mostraría nada.
    capa.set_layer(Layer::Overlay);
    // Sin anclar a ningún borde, que en layer-shell es «centrado».
    // Y con el teclado en exclusiva: es lo único que hay para escribir mientras
    // está abierto, y sin esto las teclas se las queda la ventana de atrás.
    capa.set_keyboard_mode(KeyboardMode::Exclusive);

    mudar_el_webview(&armazon, &capa)?;
    hacer_transparente(&capa);
    let _ = webview.set_background_color(Some(tauri::webview::Color(0, 0, 0, 0)));

    // Escape y la pérdida de foco se atienden acá y no en la página: la
    // superficie es la dueña del teclado, y un clic que cae en otra ventana no
    // llega nunca al WebView.
    capa.connect_key_press_event(|ventana, evento| {
        if evento.keyval() == gdk::keys::constants::Escape {
            ventana.hide();
            return glib::Propagation::Stop;
        }
        glib::Propagation::Proceed
    });

    capa.connect_focus_out_event(|ventana, _| {
        ventana.hide();
        glib::Propagation::Proceed
    });

    // Se muestra y se esconde enseguida: `show_all` es lo que realiza los
    // widgets de adentro, y sin realizarlos la primera apertura de verdad
    // volvería a pagar lo que este módulo vino a evitar.
    capa.show_all();
    capa.hide();
    armazon.hide();

    VENTANA.with(|guardada| *guardada.borrow_mut() = Some(Superficie::Capa(capa)));

    Ok(())
}

/// Lo poco que se puede pedir sin layer-shell.
///
/// Centrar y quedar encima son pedidos que el compositor puede ignorar —en
/// Wayland la ventana no elige dónde va—, pero pedirlos no cuesta nada y en un
/// escritorio que los respete la ventana queda donde corresponde.
fn montar_comun(armazon: &gtk::ApplicationWindow, webview: &tauri::WebviewWindow) {
    armazon.set_decorated(false);
    armazon.set_keep_above(true);
    armazon.set_position(gtk::WindowPosition::CenterAlways);

    // El foco perdido se atiende por el evento de Tauri y no por el de GTK: acá
    // la ventana se muestra y se esconde con la API de Tauri, y mezclar las dos
    // deja a Tauri creyendo que está visible una ventana que GTK escondió.
    //
    // El Escape no hace falta: sin superficie de capa la ventana es común y el
    // teclado le llega al WebView, así que lo atiende la propia interfaz.
    let suya = webview.clone();
    webview.on_window_event(move |evento| {
        if matches!(evento, tauri::WindowEvent::Focused(false)) {
            let _ = suya.hide();
        }
    });
}

/// Muestra la ventana y le avisa a la interfaz. Va en el hilo principal.
fn mostrar_aca(app: &AppHandle) {
    VENTANA.with(|guardada| {
        if let Some(ventana) = guardada.borrow().as_ref() {
            ventana.mostrar();
        }
    });

    // La interfaz limpia la consulta anterior y enfoca el campo. Se avisa
    // siempre que se muestra y no al construir: la ventana se construye una vez
    // y se abre cientos.
    let _ = app.emit(MOSTRADA, ());
}

fn esconder_aca() {
    VENTANA.with(|guardada| {
        if let Some(ventana) = guardada.borrow().as_ref() {
            ventana.esconder();
        }
    });
}

fn esta_visible() -> bool {
    VENTANA.with(|guardada| {
        guardada
            .borrow()
            .as_ref()
            .map(|ventana| ventana.esta_visible())
            .unwrap_or(false)
    })
}

/// Muestra la ventana desde cualquier hilo.
pub fn mostrar(app: &AppHandle) {
    let suyo = app.clone();
    let _ = app.run_on_main_thread(move || mostrar_aca(&suyo));
}

/// La esconde desde cualquier hilo.
pub fn esconder(app: &AppHandle) {
    let _ = app.run_on_main_thread(esconder_aca);
}

/// La muestra si está escondida y la esconde si está a la vista.
pub fn alternar(app: &AppHandle) {
    let suyo = app.clone();
    let _ = app.run_on_main_thread(move || {
        if esta_visible() {
            esconder_aca();
        } else {
            mostrar_aca(&suyo);
        }
    });
}

/// Saca el WebView del armazón de Tauri y lo mete en la ventana de capa.
fn mudar_el_webview(
    armazon: &gtk::ApplicationWindow,
    capa: &gtk::Window,
) -> Result<(), Box<dyn std::error::Error>> {
    let hijo = armazon.child().ok_or("la ventana de Tauri no tiene nada")?;

    let contenedor = hijo.dynamic_cast_ref::<gtk::Container>().ok_or_else(|| {
        format!(
            "lo que hay adentro no es un contenedor: {}",
            hijo.type_().name()
        )
    })?;

    let widget = contenedor
        .children()
        .first()
        .cloned()
        .ok_or("el contenedor de Tauri no tiene el WebView")?;

    contenedor.remove(&widget);
    capa.add(&widget);

    Ok(())
}

/// Sin esto la ventana de capa se dibuja con el gris de GTK detrás del WebView,
/// y el redondeado de las esquinas queda recortado sobre un rectángulo opaco.
fn hacer_transparente(capa: &gtk::Window) {
    if let Some(pantalla) = gtk::prelude::WidgetExt::screen(capa) {
        if let Some(rgba) = pantalla.rgba_visual() {
            capa.set_visual(Some(&rgba));
        }
    }

    let css = gtk::CssProvider::new();
    if css
        .load_from_data(b"window { background-color: rgba(0, 0, 0, 0); }")
        .is_ok()
    {
        capa.style_context()
            .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION + 1);
    }
}
