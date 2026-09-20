//! Los archivos que se abrieron hace poco.
//!
//! Salen de `recently-used.xbel`, que es donde GTK y Qt anotan lo que se abre:
//! no hace falta indexar nada ni vigilar nada, ya está escrito y es lo que el
//! resto del escritorio muestra en su lista de recientes.
//!
//! Va sin prefijo pero **sólo si la consulta coincide con el nombre**: son
//! archivos del usuario y aparecer sin que nadie los pida, arriba de las
//! aplicaciones, sería otra cosa.

use std::path::{Path, PathBuf};

use crate::catalogo::aplicacion::{Origen, Resultado};
use crate::catalogo::puntaje;

/// Cuánto vale un reciente frente a una aplicación.
///
/// Menos: quien escribe «terminal» quiere la terminal, no un archivo que se
/// llame así. Pero lo suficiente como para que un nombre exacto aparezca.
const PESO: f64 = 0.7;

const ICONO: &str = "document-open-recent";

/// Dónde anota el escritorio lo que se abrió.
pub fn ruta_del_archivo() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(valor) if !valor.is_empty() => PathBuf::from(valor),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/share"),
    };

    Some(base.join("recently-used.xbel"))
}

/// Un archivo reciente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reciente {
    pub ruta: String,
    /// El nombre del archivo, que es por lo que se busca.
    pub nombre: String,
}

/// Saca las rutas del XML.
///
/// A mano y no con un parser de XML: lo que hace falta es el atributo `href` de
/// cada `<bookmark>`, el formato lo fija freedesktop y no cambia, y la
/// alternativa es una dependencia entera en el arranque de un lanzador.
pub fn leer(contenido: &str) -> Vec<Reciente> {
    let mut salida = Vec::new();

    for trozo in contenido.split("<bookmark ").skip(1) {
        let Some(href) = entre_comillas(trozo, "href=\"") else {
            continue;
        };

        // Sólo archivos locales: un `sftp://` o un `trash://` no se abre desde
        // acá, y ofrecerlo es ofrecer algo que va a fallar.
        let Some(ruta) = href.strip_prefix("file://") else {
            continue;
        };

        let ruta = desescapar(ruta);
        let Some(nombre) = Path::new(&ruta).file_name() else {
            continue;
        };

        salida.push(Reciente {
            nombre: nombre.to_string_lossy().to_string(),
            ruta,
        });
    }

    salida
}

fn entre_comillas(trozo: &str, clave: &str) -> Option<String> {
    let desde = trozo.find(clave)? + clave.len();
    let hasta = trozo[desde..].find('"')? + desde;
    Some(trozo[desde..hasta].to_string())
}

/// Deshace el `%20` de las URL y las entidades del XML.
///
/// Sin esto, un archivo con un espacio en el nombre —que son la mitad— llega
/// como `mi%20archivo.txt`, se muestra así y no se abre.
fn desescapar(texto: &str) -> String {
    let bytes = texto.as_bytes();
    let mut salida = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(valor) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                salida.push(valor);
                i += 3;
                continue;
            }
        }
        salida.push(bytes[i]);
        i += 1;
    }

    String::from_utf8_lossy(&salida)
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// Los recientes que coinciden con la consulta.
pub fn buscar(recientes: &[Reciente], consulta: &str, limite: usize) -> Vec<Resultado> {
    let consulta = consulta.trim();
    if consulta.is_empty() {
        return Vec::new();
    }

    let mut filas: Vec<Resultado> = recientes
        .iter()
        .filter_map(|reciente| {
            let suyo = puntaje::puntaje(consulta, &reciente.nombre) * PESO;
            (suyo > 0.0).then(|| Resultado {
                id: reciente.ruta.clone(),
                accion: None,
                titulo: reciente.nombre.clone(),
                subtitulo: Some(reciente.ruta.clone()),
                subtitulo_dato: None,
                icono: Some(ICONO.to_string()),
                puntaje: suyo,
                origen: Origen::Reciente,
            })
        })
        .collect();

    filas.sort_by(|a, b| {
        b.puntaje
            .partial_cmp(&a.puntaje)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.titulo.cmp(&b.titulo))
    });
    filas.truncate(limite);
    filas
}

/// La lista, releída sólo cuando el archivo cambió.
///
/// El archivo tiene cientos de entradas y parsearlo en cada tecla es trabajo en
/// el camino crítico de la consulta. Pero tampoco se puede leer una vez al
/// arrancar y olvidarse: el escritorio lo reescribe cada vez que se abre algo,
/// y un lanzador que vive prendido se quedaría con la lista del día que se
/// inició la sesión.
///
/// Así que se mira la fecha del archivo, que es una llamada al sistema de
/// microsegundos, y se relee sólo si cambió.
#[derive(Default)]
pub struct Cache {
    fecha: Option<i64>,
    lista: Vec<Reciente>,
}

impl Cache {
    pub fn nueva() -> Self {
        let mut cache = Self::default();
        cache.actualizar();
        cache
    }

    /// La lista al día.
    pub fn lista(&mut self) -> &[Reciente] {
        self.actualizar();
        &self.lista
    }

    fn actualizar(&mut self) {
        let Some(ruta) = ruta_del_archivo() else {
            return;
        };

        let fecha = std::fs::metadata(&ruta)
            .and_then(|datos| datos.modified())
            .ok()
            .and_then(|momento| momento.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|desde| desde.as_secs() as i64);

        // Que el archivo no esté es normal: una sesión nueva no abrió nada
        // todavía. Ahí la lista queda vacía y se vuelve a mirar la próxima.
        if fecha == self.fecha && fecha.is_some() {
            return;
        }

        self.fecha = fecha;
        self.lista = std::fs::read_to_string(&ruta)
            .map(|contenido| leer(&contenido))
            .unwrap_or_default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XBEL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<xbel version="1.0">
  <bookmark href="file:///home/pato/Documentos/informe%20final.odt" added="2026-09-01T10:00:00Z">
    <info><metadata owner="http://freedesktop.org"/></info>
  </bookmark>
  <bookmark href="file:///home/pato/notas.md" added="2026-09-02T10:00:00Z">
    <info><metadata owner="http://freedesktop.org"/></info>
  </bookmark>
  <bookmark href="sftp://servidor/remoto.txt" added="2026-09-03T10:00:00Z">
    <info><metadata owner="http://freedesktop.org"/></info>
  </bookmark>
</xbel>"#;

    #[test]
    fn se_leen_las_rutas() {
        let leidos = leer(XBEL);
        assert_eq!(leidos.len(), 2);
        assert_eq!(leidos[1].nombre, "notas.md");
        assert_eq!(leidos[1].ruta, "/home/pato/notas.md");
    }

    #[test]
    fn los_espacios_del_nombre_vuelven_a_ser_espacios() {
        // Sin deshacer el `%20`, el archivo se muestra como
        // «informe%20final.odt» y al abrirlo no existe.
        let leidos = leer(XBEL);
        assert_eq!(leidos[0].nombre, "informe final.odt");
        assert_eq!(leidos[0].ruta, "/home/pato/Documentos/informe final.odt");
    }

    #[test]
    fn lo_que_no_es_un_archivo_local_no_entra() {
        // Un `sftp://` no se abre desde acá: ofrecerlo es ofrecer algo que falla.
        assert!(leer(XBEL).iter().all(|uno| !uno.ruta.contains("servidor")));
    }

    #[test]
    fn las_entidades_del_xml_se_deshacen() {
        let contenido = r#"<bookmark href="file:///home/pato/uno%20%26%20otro.txt" added="x">"#;
        assert_eq!(leer(contenido)[0].nombre, "uno & otro.txt");
    }

    #[test]
    fn un_archivo_sin_nada_adentro_no_rompe() {
        assert!(leer("").is_empty());
        assert!(leer("<xbel></xbel>").is_empty());
        // Un bookmark a medias se saltea en vez de tirar todo abajo.
        assert!(leer("<bookmark ").is_empty());
    }

    #[test]
    fn se_busca_por_el_nombre() {
        let recientes = leer(XBEL);
        let filas = buscar(&recientes, "notas", 10);
        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].titulo, "notas.md");
        assert_eq!(filas[0].id, "/home/pato/notas.md");
    }

    #[test]
    fn con_el_campo_vacio_no_aparecen() {
        // Son archivos del usuario: aparecer sin que nadie los pida, arriba de
        // las aplicaciones, sería otra cosa.
        assert!(buscar(&leer(XBEL), "", 10).is_empty());
    }

    #[test]
    fn la_cache_relee_cuando_el_archivo_cambia() {
        // Un lanzador que vive prendido no puede quedarse con la lista del día
        // que se inició la sesión.
        let ruta =
            std::env::temp_dir().join(format!("prism-recientes-{}.xbel", std::process::id()));
        let anterior = std::env::var_os("XDG_DATA_HOME");
        // La caché mira `$XDG_DATA_HOME/recently-used.xbel`, así que el
        // directorio de prueba tiene que ser ése.
        let directorio = ruta
            .parent()
            .unwrap()
            .join(format!("prism-xdg-{}", std::process::id()));
        std::fs::create_dir_all(&directorio).unwrap();
        let archivo = directorio.join("recently-used.xbel");
        std::env::set_var("XDG_DATA_HOME", &directorio);

        std::fs::write(
            &archivo,
            r#"<bookmark href="file:///home/pato/uno.txt" added="x">"#,
        )
        .unwrap();
        let mut cache = Cache::nueva();
        assert_eq!(cache.lista().len(), 1);

        // La fecha tiene segundos de resolución: sin esperar, el cambio no se ve.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(
            &archivo,
            r#"<bookmark href="file:///home/pato/uno.txt" added="x"><bookmark href="file:///home/pato/dos.txt" added="x">"#,
        )
        .unwrap();

        assert_eq!(cache.lista().len(), 2);

        match anterior {
            Some(valor) => std::env::set_var("XDG_DATA_HOME", valor),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        let _ = std::fs::remove_dir_all(&directorio);
    }

    #[test]
    fn valen_menos_que_una_aplicacion() {
        // Quien escribe «terminal» quiere la terminal, no un archivo así.
        let recientes = vec![Reciente {
            nombre: "notas.md".to_string(),
            ruta: "/home/pato/notas.md".to_string(),
        }];
        let fila = buscar(&recientes, "notas.md", 10).pop().unwrap();
        assert!(fila.puntaje < 100.0);
    }
}
