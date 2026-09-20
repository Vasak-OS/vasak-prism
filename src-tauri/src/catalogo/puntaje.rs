//! Cuánto se parece lo que se escribió a lo que hay.
//!
//! Dos cosas que el puntaje del escritorio hacía mal y que se notan escribiendo
//! en español:
//!
//!  1. Comparaba `matches == consulta.len()`, que son **caracteres contra
//!     bytes**. Cualquier consulta con acento o con «ñ» nunca llegaba a puntuar
//!     por coincidencia difusa, porque el contador jamás alcanzaba el largo.
//!  2. No plegaba los acentos, así que «busqueda» no encontraba «Búsqueda» y
//!     «cafe» no encontraba «Café». Nadie escribe los acentos en un buscador.

/// El techo: el texto es exactamente lo que se escribió.
const EXACTO: f64 = 100.0;
/// El texto empieza con lo escrito. Es lo que uno espera al escribir tres letras.
const EMPIEZA: f64 = 90.0;
/// Alguna palabra del texto empieza con lo escrito: «fire» para «Mozilla Firefox».
const PALABRA: f64 = 80.0;
/// El texto contiene lo escrito en algún lado.
const CONTIENE: f64 = 65.0;
/// Las letras aparecen en orden pero salteadas. El piso, y con castigo.
const SALTEADO: f64 = 50.0;

/// Pasa un texto a la forma en la que se compara: minúsculas y sin acentos.
pub fn normalizar(texto: &str) -> String {
    texto.chars().flat_map(plegar).collect()
}

/// Un carácter en minúscula y sin marca diacrítica.
///
/// Devuelve un iterador porque la ß minúscula es una letra y el plegado de
/// algunas mayúsculas da dos caracteres; `to_lowercase` ya lo contempla y no
/// hay motivo para perderlo.
fn plegar(c: char) -> impl Iterator<Item = char> {
    let sin_acento = match c {
        'á' | 'à' | 'ä' | 'â' | 'ã' | 'å' | 'Á' | 'À' | 'Ä' | 'Â' | 'Ã' | 'Å' => 'a',
        'é' | 'è' | 'ë' | 'ê' | 'É' | 'È' | 'Ë' | 'Ê' => 'e',
        'í' | 'ì' | 'ï' | 'î' | 'Í' | 'Ì' | 'Ï' | 'Î' => 'i',
        'ó' | 'ò' | 'ö' | 'ô' | 'õ' | 'Ó' | 'Ò' | 'Ö' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ù' | 'ü' | 'û' | 'Ú' | 'Ù' | 'Ü' | 'Û' => 'u',
        // La eñe se pliega igual que el resto. «Año» y «ano» se confunden, y es
        // preferible a que quien escribe «ano» no encuentre «Año»: en un
        // buscador nadie usa el teclado muerto.
        'ñ' | 'Ñ' => 'n',
        'ç' | 'Ç' => 'c',
        otro => otro,
    };

    sin_acento.to_lowercase()
}

/// Cuánto se parece `consulta` a `texto`, de 0 a 100.
///
/// Los dos se normalizan acá: quien llama no tiene que acordarse, y olvidarse
/// es un error que no da ningún síntoma más que resultados que no aparecen.
pub fn puntaje(consulta: &str, texto: &str) -> f64 {
    let consulta = normalizar(consulta.trim());
    let texto = normalizar(texto);

    if consulta.is_empty() || texto.is_empty() {
        return 0.0;
    }

    if texto == consulta {
        return EXACTO;
    }

    if texto.starts_with(&consulta) {
        return EMPIEZA;
    }

    // Por palabra, y no sólo desde el principio: las aplicaciones se nombran
    // «Mozilla Firefox» o «Editor de textos», y nadie escribe la primera.
    if texto
        .split(|c: char| c.is_whitespace() || c == '-' || c == '_')
        .any(|palabra| !palabra.is_empty() && palabra.starts_with(&consulta))
    {
        return PALABRA;
    }

    if texto.contains(&consulta) {
        return CONTIENE;
    }

    salteado(&consulta, &texto)
}

/// Las letras en orden pero no seguidas: «gimp» para «GNU Image Manipulation».
///
/// Se trabaja sobre caracteres, no sobre bytes. Ése era el error: con `len()`
/// el contador se comparaba contra la cantidad de bytes de la consulta, y una
/// consulta con un solo acento ya tenía un byte de más.
fn salteado(consulta: &str, texto: &str) -> f64 {
    let texto: Vec<char> = texto.chars().collect();
    let mut posicion = 0;
    let mut encontradas = 0;
    let mut recorrido = 0;

    for buscada in consulta.chars() {
        let Some(salto) = texto[posicion..].iter().position(|c| *c == buscada) else {
            return 0.0;
        };
        posicion += salto + 1;
        recorrido += salto;
        encontradas += 1;
    }

    if encontradas != consulta.chars().count() {
        return 0.0;
    }

    // Cuanto más desparramadas estén las letras, menos se parece: «gmp» en
    // «GNU Image Manipulation» vale menos que en «gimp».
    SALTEADO / (1.0 + recorrido as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_texto_igual_puntua_al_maximo() {
        assert_eq!(puntaje("firefox", "Firefox"), EXACTO);
    }

    #[test]
    fn empezar_vale_mas_que_contener() {
        assert!(puntaje("fire", "Firefox") > puntaje("fox", "Firefox"));
    }

    #[test]
    fn una_palabra_del_medio_vale_mas_que_una_letra_suelta() {
        // Nadie escribe «mozilla» para abrir Firefox.
        assert_eq!(puntaje("fire", "Mozilla Firefox"), PALABRA);
        assert!(puntaje("fire", "Mozilla Firefox") > puntaje("zilla", "Mozilla Firefox"));
    }

    #[test]
    fn los_acentos_no_hacen_falta() {
        // Lo que no andaba: nadie escribe los acentos en un buscador.
        assert_eq!(puntaje("busqueda", "Búsqueda"), EXACTO);
        assert_eq!(puntaje("cafe", "Café"), EXACTO);
        assert_eq!(puntaje("configuracion", "Configuración"), EXACTO);
    }

    #[test]
    fn y_ponerlos_tampoco_molesta() {
        assert_eq!(puntaje("búsqueda", "Busqueda"), EXACTO);
    }

    #[test]
    fn una_consulta_con_acento_llega_a_puntuar_por_salteado() {
        // El error de bytes contra caracteres: acá el contador nunca alcanzaba
        // el largo de la consulta y la coincidencia difusa devolvía 0.
        assert!(puntaje("cmra", "Cámara fotográfica") > 0.0);
        assert!(puntaje("añ", "Calendario del año") > 0.0);
    }

    #[test]
    fn las_letras_en_orden_pero_salteadas_puntuan_poco_y_puntuan() {
        let salteada = puntaje("gmp", "GNU Image Manipulation Program");
        assert!(salteada > 0.0);
        assert!(salteada < CONTIENE);
    }

    #[test]
    fn cuanto_mas_juntas_las_letras_mas_vale() {
        assert!(puntaje("gim", "gimp x") > puntaje("gim", "g i m"));
    }

    #[test]
    fn lo_que_no_esta_no_puntua() {
        assert_eq!(puntaje("zzz", "Firefox"), 0.0);
        // Las letras están, pero no en orden.
        assert_eq!(puntaje("xof", "Firefox"), 0.0);
    }

    #[test]
    fn una_consulta_vacia_no_puntua() {
        assert_eq!(puntaje("", "Firefox"), 0.0);
        assert_eq!(puntaje("   ", "Firefox"), 0.0);
        assert_eq!(puntaje("firefox", ""), 0.0);
    }

    #[test]
    fn normalizar_deja_el_texto_comparable() {
        assert_eq!(normalizar("Añejo Café"), "anejo cafe");
    }
}
