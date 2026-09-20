//! Buscar un emoji por su nombre y copiarlo.
//!
//! Con `:` adelante, como se escriben en media internet. Es de las cosas que
//! más se agradecen de un lanzador: el selector del escritorio no existe, y la
//! alternativa es buscarlo en el navegador y copiarlo de ahí.
//!
//! La tabla está escrita a mano y no sacada del Unicode entero, que son más de
//! tres mil. Los nombres oficiales están sólo en inglés —«smiling face with
//! smiling eyes»— y nadie escribe eso: quien busca la cara feliz escribe
//! «feliz», «sonrisa» o «smile». Esto es lo que se usa, con las dos lenguas.

use crate::catalogo::aplicacion::{Origen, Resultado};

use super::puntaje_de_prefijo;

/// Un emoji y con qué palabras se lo encuentra.
struct Emoji {
    /// El carácter, que es lo que se copia.
    caracter: &'static str,
    /// Cómo se llama. El primero es el que se muestra.
    nombres: &'static [&'static str],
}

const EMOJIS: &[Emoji] = &[
    // Caras
    Emoji {
        caracter: "😀",
        nombres: &["sonrisa", "feliz", "smile", "grin"],
    },
    Emoji {
        caracter: "😂",
        nombres: &["risa", "llorar de risa", "lol", "joy"],
    },
    Emoji {
        caracter: "🙂",
        nombres: &["sonrisa leve", "slight smile"],
    },
    Emoji {
        caracter: "😉",
        nombres: &["guiño", "wink"],
    },
    Emoji {
        caracter: "😍",
        nombres: &["enamorado", "corazones", "heart eyes"],
    },
    Emoji {
        caracter: "😎",
        nombres: &["lentes", "cool", "sunglasses"],
    },
    Emoji {
        caracter: "🤔",
        nombres: &["pensando", "duda", "thinking"],
    },
    Emoji {
        caracter: "😐",
        nombres: &["neutral", "sin expresion"],
    },
    Emoji {
        caracter: "😴",
        nombres: &["dormido", "sueño", "sleep"],
    },
    Emoji {
        caracter: "😭",
        nombres: &["llorando", "llanto", "cry"],
    },
    Emoji {
        caracter: "😡",
        nombres: &["enojado", "bronca", "angry"],
    },
    Emoji {
        caracter: "🥳",
        nombres: &["fiesta", "festejo", "party"],
    },
    Emoji {
        caracter: "😅",
        nombres: &["nervioso", "sweat smile"],
    },
    Emoji {
        caracter: "🤡",
        nombres: &["payaso", "clown"],
    },
    Emoji {
        caracter: "💀",
        nombres: &["calavera", "muerto", "skull"],
    },
    Emoji {
        caracter: "👻",
        nombres: &["fantasma", "ghost"],
    },
    Emoji {
        caracter: "🤖",
        nombres: &["robot", "bot"],
    },
    Emoji {
        caracter: "👽",
        nombres: &["alien", "extraterrestre"],
    },
    // Manos y gente
    Emoji {
        caracter: "👍",
        nombres: &["pulgar arriba", "bien", "ok", "thumbs up"],
    },
    Emoji {
        caracter: "👎",
        nombres: &["pulgar abajo", "mal", "thumbs down"],
    },
    Emoji {
        caracter: "👏",
        nombres: &["aplauso", "aplausos", "clap"],
    },
    Emoji {
        caracter: "🙏",
        nombres: &["gracias", "por favor", "rezar", "pray"],
    },
    Emoji {
        caracter: "🤝",
        nombres: &["apreton de manos", "trato", "handshake"],
    },
    Emoji {
        caracter: "💪",
        nombres: &["fuerza", "musculo", "muscle"],
    },
    Emoji {
        caracter: "👋",
        nombres: &["hola", "saludo", "chau", "wave"],
    },
    Emoji {
        caracter: "🫡",
        nombres: &["saludo militar", "salute"],
    },
    Emoji {
        caracter: "🤷",
        nombres: &["que se yo", "encogerse", "shrug"],
    },
    // Corazones y símbolos
    Emoji {
        caracter: "❤️",
        nombres: &["corazon", "amor", "heart"],
    },
    Emoji {
        caracter: "💔",
        nombres: &["corazon roto", "broken heart"],
    },
    Emoji {
        caracter: "🔥",
        nombres: &["fuego", "fire"],
    },
    Emoji {
        caracter: "⭐",
        nombres: &["estrella", "star"],
    },
    Emoji {
        caracter: "✨",
        nombres: &["brillos", "chispas", "sparkles"],
    },
    Emoji {
        caracter: "⚡",
        nombres: &["rayo", "electricidad", "zap"],
    },
    Emoji {
        caracter: "💡",
        nombres: &["idea", "lampara", "bulb"],
    },
    Emoji {
        caracter: "✅",
        nombres: &["listo", "hecho", "tilde", "check"],
    },
    Emoji {
        caracter: "❌",
        nombres: &["error", "cruz", "no", "cross"],
    },
    Emoji {
        caracter: "⚠️",
        nombres: &["advertencia", "cuidado", "warning"],
    },
    Emoji {
        caracter: "🚀",
        nombres: &["cohete", "lanzamiento", "rocket"],
    },
    Emoji {
        caracter: "🎉",
        nombres: &["festejo", "celebracion", "tada"],
    },
    Emoji {
        caracter: "🐛",
        nombres: &["bicho", "error", "bug"],
    },
    Emoji {
        caracter: "🔒",
        nombres: &["candado", "cerrado", "lock"],
    },
    Emoji {
        caracter: "🔑",
        nombres: &["llave", "clave", "key"],
    },
    Emoji {
        caracter: "📌",
        nombres: &["chinche", "fijar", "pin"],
    },
    Emoji {
        caracter: "🔍",
        nombres: &["lupa", "buscar", "search"],
    },
    Emoji {
        caracter: "🗑️",
        nombres: &["basura", "borrar", "trash"],
    },
    Emoji {
        caracter: "📎",
        nombres: &["clip", "adjunto", "paperclip"],
    },
    Emoji {
        caracter: "✏️",
        nombres: &["lapiz", "editar", "pencil"],
    },
    Emoji {
        caracter: "📝",
        nombres: &["nota", "escribir", "memo"],
    },
    Emoji {
        caracter: "📅",
        nombres: &["calendario", "fecha", "calendar"],
    },
    Emoji {
        caracter: "⏰",
        nombres: &["reloj", "alarma", "hora", "clock"],
    },
    Emoji {
        caracter: "📧",
        nombres: &["correo", "mail", "email"],
    },
    Emoji {
        caracter: "💬",
        nombres: &["mensaje", "chat", "globo"],
    },
    // Cosas
    Emoji {
        caracter: "☕",
        nombres: &["cafe", "coffee"],
    },
    Emoji {
        caracter: "🍺",
        nombres: &["cerveza", "birra", "beer"],
    },
    Emoji {
        caracter: "🍕",
        nombres: &["pizza"],
    },
    Emoji {
        caracter: "🎂",
        nombres: &["torta", "cumpleaños", "cake"],
    },
    Emoji {
        caracter: "🌙",
        nombres: &["luna", "noche", "moon"],
    },
    Emoji {
        caracter: "☀️",
        nombres: &["sol", "dia", "sun"],
    },
    Emoji {
        caracter: "🌧️",
        nombres: &["lluvia", "rain"],
    },
    Emoji {
        caracter: "🐧",
        nombres: &["pinguino", "linux", "penguin"],
    },
    Emoji {
        caracter: "🖥️",
        nombres: &["computadora", "pantalla", "desktop"],
    },
    Emoji {
        caracter: "💻",
        nombres: &["notebook", "laptop"],
    },
    Emoji {
        caracter: "📱",
        nombres: &["telefono", "celular", "phone"],
    },
    Emoji {
        caracter: "🎵",
        nombres: &["musica", "nota musical", "music"],
    },
    Emoji {
        caracter: "📷",
        nombres: &["camara", "foto", "camera"],
    },
    Emoji {
        caracter: "🏠",
        nombres: &["casa", "hogar", "home"],
    },
    Emoji {
        caracter: "🚗",
        nombres: &["auto", "coche", "car"],
    },
    Emoji {
        caracter: "✈️",
        nombres: &["avion", "vuelo", "plane"],
    },
    // Flechas y signos que no son emoji pero se buscan igual
    Emoji {
        caracter: "→",
        nombres: &["flecha derecha", "arrow right"],
    },
    Emoji {
        caracter: "←",
        nombres: &["flecha izquierda", "arrow left"],
    },
    Emoji {
        caracter: "↑",
        nombres: &["flecha arriba", "arrow up"],
    },
    Emoji {
        caracter: "↓",
        nombres: &["flecha abajo", "arrow down"],
    },
    Emoji {
        caracter: "—",
        nombres: &["raya", "guion largo", "em dash"],
    },
    Emoji {
        caracter: "…",
        nombres: &["puntos suspensivos", "ellipsis"],
    },
    Emoji {
        caracter: "«",
        nombres: &["comilla angular abre", "guillemet"],
    },
    Emoji {
        caracter: "»",
        nombres: &["comilla angular cierra"],
    },
    Emoji {
        caracter: "°",
        nombres: &["grado", "grados", "degree"],
    },
    Emoji {
        caracter: "€",
        nombres: &["euro"],
    },
    Emoji {
        caracter: "±",
        nombres: &["mas menos", "plus minus"],
    },
    Emoji {
        caracter: "≠",
        nombres: &["distinto", "no igual", "not equal"],
    },
    Emoji {
        caracter: "✓",
        nombres: &["tilde", "visto", "check mark"],
    },
];

/// Los emojis que coinciden con lo escrito.
pub fn buscar(consulta: &str, limite: usize) -> Vec<Resultado> {
    let consulta = consulta.trim();

    let mut filas: Vec<(f64, Resultado)> = EMOJIS
        .iter()
        .filter_map(|emoji| {
            // Sin consulta se muestran todos: abrir `:` y ver qué hay es una
            // forma legítima de usarlo.
            let puntaje = if consulta.is_empty() {
                1.0
            } else {
                emoji
                    .nombres
                    .iter()
                    .map(|nombre| crate::catalogo::puntaje::puntaje(consulta, nombre))
                    .fold(0.0_f64, f64::max)
            };

            (puntaje > 0.0).then(|| {
                (
                    puntaje,
                    Resultado {
                        // Lo que se copia.
                        id: emoji.caracter.to_string(),
                        accion: None,
                        titulo: format!("{}  {}", emoji.caracter, emoji.nombres[0]),
                        subtitulo: Some("lanzador.copiar".to_string()),
                        subtitulo_dato: None,
                        // Sin icono: el emoji **es** el icono, y poner uno al
                        // lado lo dejaría compitiendo con él.
                        icono: None,
                        puntaje: puntaje_de_prefijo(puntaje),
                        origen: Origen::Emoji,
                    },
                )
            })
        })
        .collect();

    filas.sort_by(|(a, uno), (b, otro)| {
        b.partial_cmp(a)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| uno.titulo.cmp(&otro.titulo))
    });

    filas
        .into_iter()
        .take(limite)
        .map(|(_, fila)| fila)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn caracteres(consulta: &str) -> Vec<String> {
        buscar(consulta, 10)
            .into_iter()
            .map(|fila| fila.id)
            .collect()
    }

    #[test]
    fn se_busca_en_español() {
        // Los nombres oficiales de Unicode están sólo en inglés, y nadie escribe
        // «smiling face with smiling eyes».
        assert!(caracteres("corazon").contains(&"❤️".to_string()));
        assert!(caracteres("fuego").contains(&"🔥".to_string()));
        assert!(caracteres("pinguino").contains(&"🐧".to_string()));
    }

    #[test]
    fn y_en_ingles_tambien() {
        assert!(caracteres("fire").contains(&"🔥".to_string()));
        assert!(caracteres("rocket").contains(&"🚀".to_string()));
    }

    #[test]
    fn los_acentos_no_hacen_falta() {
        // Va por el mismo puntaje que todo lo demás, que pliega los acentos.
        assert!(caracteres("guiño").contains(&"😉".to_string()));
        assert!(caracteres("guino").contains(&"😉".to_string()));
    }

    #[test]
    fn se_copia_el_caracter_y_se_muestra_con_su_nombre() {
        let fila = buscar("fuego", 1).pop().expect("hay uno");
        assert_eq!(fila.id, "🔥");
        assert!(fila.titulo.contains("🔥"));
        assert!(fila.titulo.contains("fuego"));
    }

    #[test]
    fn el_emoji_no_lleva_icono_al_lado() {
        // El emoji **es** el icono; otro al lado competiría con él.
        assert_eq!(buscar("fuego", 1)[0].icono, None);
    }

    #[test]
    fn sin_consulta_se_muestran_todos() {
        // Abrir `:` y ver qué hay es una forma legítima de usarlo.
        assert_eq!(buscar("", 10).len(), 10);
        assert!(!buscar("", 500).is_empty());
    }

    #[test]
    fn lo_que_no_esta_no_inventa_nada() {
        assert!(caracteres("zzzzz").is_empty());
    }

    #[test]
    fn hay_signos_que_no_son_emoji_pero_se_buscan_igual() {
        // La raya y los puntos suspensivos no están en ningún teclado y se usan
        // en cada párrafo.
        assert!(caracteres("raya").contains(&"—".to_string()));
        assert!(caracteres("grados").contains(&"°".to_string()));
    }

    #[test]
    fn el_limite_se_respeta() {
        assert!(buscar("a", 3).len() <= 3);
    }
}
