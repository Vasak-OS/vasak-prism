//! El servicio de D-Bus: cómo se le pide al lanzador que aparezca.
//!
//! El atajo del teclado no arranca un programa nuevo —eso es justo lo que este
//! rediseño saca—: le habla al que ya está corriendo. Y el nombre en el bus es
//! además lo que hace de instancia única, sin ningún archivo de bloqueo: el
//! segundo que intente tomarlo se entera de que hay otro y le pasa el pedido.

use tauri::AppHandle;
use zbus::{interface, Connection};

/// El nombre que el daemon toma en el bus de sesión.
pub const NOMBRE: &str = "ar.net.vasak.Prism";
/// Dónde vive el objeto.
pub const RUTA: &str = "/ar/net/vasak/Prism";

struct Servicio {
    app: AppHandle,
}

#[interface(name = "ar.net.vasak.Prism")]
impl Servicio {
    /// Lo que llama el atajo del teclado.
    fn toggle(&self) {
        crate::ventana::alternar(&self.app);
    }

    fn show(&self) {
        crate::ventana::mostrar(&self.app);
    }

    fn hide(&self) {
        crate::ventana::esconder(&self.app);
    }
}

/// Toma el nombre en el bus y queda atendiendo.
///
/// Si el nombre ya está tomado, devuelve error: hay otra instancia y quien
/// llamó tiene que pasarle el pedido en vez de abrir una segunda ventana.
pub async fn servir(app: AppHandle) -> zbus::Result<Connection> {
    let conexion = zbus::connection::Builder::session()?
        .name(NOMBRE)?
        .serve_at(RUTA, Servicio { app })?
        .build()
        .await?;

    Ok(conexion)
}

/// Le pide a la instancia que ya está corriendo que muestre u oculte la ventana.
///
/// Bloqueante porque corre antes de que exista ningún runtime: es lo primero
/// que hace el programa al arrancar, para decidir si tiene que ser el daemon o
/// alcanzaba con avisarle al que estaba.
///
/// Devuelve `false` si no hay nadie: ahí hay que arrancar.
pub fn pedir_alternar() -> bool {
    let Ok(conexion) = zbus::blocking::Connection::session() else {
        return false;
    };

    conexion
        .call_method(Some(NOMBRE), RUTA, Some(NOMBRE), "Toggle", &())
        .is_ok()
}
