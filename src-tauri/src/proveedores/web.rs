//! Buscar en la web sin abrir antes el navegador.
//!
//! Con `?` adelante va al buscador por omisión, y con un *bang* al sitio que
//! diga: `!w` a Wikipedia, `!aur` al AUR. La idea es de DuckDuckGo y se copia a
//! propósito — quien ya los usa no tiene que aprender nada.

use crate::catalogo::aplicacion::{Origen, Resultado};

/// Dónde va lo que no lleva bang.
///
/// Fijo por ahora. Debería salir de la configuración del escritorio, y no lo
/// hace todavía porque el plugin de configuración por debajo de la 2.6.0 borra
/// de `vasak.conf` las claves que no conoce: agregar una nueva desde acá es
/// arriesgarse a que se la lleve puesta otra aplicación con el plugin viejo.
const POR_OMISION: &str = "https://duckduckgo.com/?q=";

const ICONO: &str = "internet-web-browser";

struct Bang {
    /// Sin el `!`.
    nombre: &'static str,
    /// Cómo se lo llama en la lista.
    sitio: &'static str,
    /// La dirección, con la consulta pegada al final.
    url: &'static str,
}

const BANGS: &[Bang] = &[
    Bang {
        nombre: "g",
        sitio: "Google",
        url: "https://www.google.com/search?q=",
    },
    Bang {
        nombre: "ddg",
        sitio: "DuckDuckGo",
        url: "https://duckduckgo.com/?q=",
    },
    Bang {
        nombre: "w",
        sitio: "Wikipedia",
        url: "https://es.wikipedia.org/w/index.php?search=",
    },
    Bang {
        nombre: "wen",
        sitio: "Wikipedia (en)",
        url: "https://en.wikipedia.org/w/index.php?search=",
    },
    Bang {
        nombre: "aur",
        sitio: "AUR",
        url: "https://aur.archlinux.org/packages?K=",
    },
    Bang {
        nombre: "arch",
        sitio: "Arch Wiki",
        url: "https://wiki.archlinux.org/index.php?search=",
    },
    Bang {
        nombre: "gh",
        sitio: "GitHub",
        url: "https://github.com/search?q=",
    },
    Bang {
        nombre: "crates",
        sitio: "crates.io",
        url: "https://crates.io/search?q=",
    },
    Bang {
        nombre: "docs",
        sitio: "docs.rs",
        url: "https://docs.rs/releases/search?query=",
    },
    Bang {
        nombre: "npm",
        sitio: "npm",
        url: "https://www.npmjs.com/search?q=",
    },
    Bang {
        nombre: "yt",
        sitio: "YouTube",
        url: "https://www.youtube.com/results?search_query=",
    },
    Bang {
        nombre: "mdn",
        sitio: "MDN",
        url: "https://developer.mozilla.org/search?q=",
    },
];

/// Codifica para meter en una URL.
///
/// A mano y no con una biblioteca: son veinte líneas y la alternativa es una
/// dependencia más en el arranque de un lanzador. Lo que no se toca es lo que
/// el estándar llama «sin reservar»; todo lo demás va en porcentaje, que es lo
/// seguro. Sin esto, buscar `a & b` corta la consulta en el `&` y el sitio
/// recibe otra cosa.
pub fn codificar(texto: &str) -> String {
    let mut salida = String::with_capacity(texto.len());

    for byte in texto.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                salida.push(*byte as char)
            }
            // El espacio va como `+`, que es lo que esperan los buscadores en la
            // parte de la consulta.
            b' ' => salida.push('+'),
            otro => salida.push_str(&format!("%{otro:02X}")),
        }
    }

    salida
}

/// La fila para buscar en la web, si la consulta la pide.
pub fn buscar(consulta: &str) -> Vec<Resultado> {
    let consulta = consulta.trim();

    // Un bang: `!w algo`. El bang solo, sin nada detrás, no busca nada —pero se
    // ofrece igual, así se ve la lista escribiendo `?!`.
    if let Some(resto) = consulta.strip_prefix('!') {
        let (nombre, termino) = match resto.split_once(char::is_whitespace) {
            Some((nombre, termino)) => (nombre, termino.trim()),
            None => (resto, ""),
        };

        return match BANGS
            .iter()
            .find(|bang| bang.nombre == nombre.to_lowercase())
        {
            Some(bang) if !termino.is_empty() => vec![fila(bang.sitio, bang.url, termino)],
            // Un bang que no existe o sin término: se ofrecen los que hay, para
            // no dejar la lista vacía justo cuando alguien está tanteando.
            _ => sugerencias(nombre),
        };
    }

    if consulta.is_empty() {
        return sugerencias("");
    }

    vec![fila("DuckDuckGo", POR_OMISION, consulta)]
}

fn fila(sitio: &str, base: &str, termino: &str) -> Resultado {
    Resultado {
        // La dirección completa: es lo que se abre.
        id: format!("{base}{}", codificar(termino)),
        accion: None,
        titulo: termino.to_string(),
        subtitulo: Some("lanzador.buscarEn".to_string()),
        subtitulo_dato: Some(sitio.to_string()),
        icono: Some(ICONO.to_string()),
        puntaje: super::puntaje_de_prefijo(100.0),
        origen: Origen::Web,
    }
}

/// Los bangs que hay, para cuando todavía no se escribió uno entero.
fn sugerencias(empezado: &str) -> Vec<Resultado> {
    let empezado = empezado.to_lowercase();

    BANGS
        .iter()
        .filter(|bang| bang.nombre.starts_with(&empezado))
        .map(|bang| Resultado {
            // Sin término no hay nada que abrir: al elegirlo se completa el
            // bang en el campo, que es lo que hace quien lo estaba tanteando.
            id: format!("!{} ", bang.nombre),
            accion: None,
            titulo: format!("!{}", bang.nombre),
            subtitulo: Some("lanzador.buscarEn".to_string()),
            subtitulo_dato: Some(bang.sitio.to_string()),
            icono: Some(ICONO.to_string()),
            puntaje: super::puntaje_de_prefijo(50.0),
            origen: Origen::Completar,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lo_escrito_va_al_buscador_por_omision() {
        let filas = buscar("gatos");
        assert_eq!(filas.len(), 1);
        assert!(filas[0].id.starts_with("https://duckduckgo.com/"));
        assert!(filas[0].id.ends_with("gatos"));
        assert_eq!(filas[0].titulo, "gatos");
    }

    #[test]
    fn un_bang_manda_a_su_sitio() {
        let filas = buscar("!w pinguino");
        assert_eq!(filas.len(), 1);
        assert!(filas[0].id.starts_with("https://es.wikipedia.org/"));
        assert!(filas[0].id.ends_with("pinguino"));
    }

    #[test]
    fn el_espacio_y_los_signos_no_rompen_la_direccion() {
        // Sin codificar, `a & b` corta la consulta en el `&` y el sitio recibe
        // otra cosa.
        assert_eq!(codificar("a & b"), "a+%26+b");
        assert_eq!(codificar("c++"), "c%2B%2B");
        assert_eq!(codificar("100%"), "100%25");
        // Y lo que no hay que tocar no se toca.
        assert_eq!(codificar("abc-_.~123"), "abc-_.~123");
    }

    #[test]
    fn los_acentos_salen_en_utf8() {
        assert_eq!(codificar("ñ"), "%C3%B1");
    }

    #[test]
    fn un_bang_a_medias_ofrece_los_que_hay() {
        // Para no dejar la lista vacía justo cuando alguien está tanteando.
        let filas = buscar("!w");
        assert!(filas.len() >= 2);
        assert!(filas.iter().all(|fila| fila.origen == Origen::Completar));
        assert!(filas.iter().any(|fila| fila.titulo == "!w"));
        assert!(filas.iter().any(|fila| fila.titulo == "!wen"));
    }

    #[test]
    fn un_bang_que_no_existe_no_manda_a_ningun_lado() {
        let filas = buscar("!nohay algo");
        assert!(filas.iter().all(|fila| fila.origen == Origen::Completar));
        assert!(filas.is_empty() || !filas[0].id.starts_with("http"));
    }

    #[test]
    fn el_bang_no_distingue_mayusculas() {
        assert!(buscar("!W pinguino")[0]
            .id
            .starts_with("https://es.wikipedia.org/"));
    }

    #[test]
    fn sin_nada_escrito_se_ve_la_lista_de_bangs() {
        let filas = buscar("");
        assert_eq!(filas.len(), BANGS.len());
    }
}
