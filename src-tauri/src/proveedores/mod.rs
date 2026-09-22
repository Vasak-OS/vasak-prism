//! Lo que contesta además de las aplicaciones.
//!
//! # Cómo se reparte la consulta
//!
//! Un **prefijo manda**: si lo escrito arranca con `>`, `?` o `:`, contesta ese
//! proveedor y nadie más. Es una elección explícita de quien escribe, y mezclar
//! aplicaciones ahí abajo sería ruido — además de que `:` y `>` aparecen
//! adentro de texto normal todo el tiempo, y sin la regla del prefijo habría
//! que adivinar.
//!
//! Sin prefijo contestan los tres que se pueden decidir mirando lo escrito: la
//! cuenta, las aplicaciones y los recientes. La cuenta no dispara si lo escrito
//! no es una, y los recientes sólo si el nombre coincide.
//!
//! # Y los prefijos de palabra
//!
//! Los de arriba son de un carácter y existen porque sin ellos su proveedor no
//! se podría disparar nunca. Los otros tres —aplicaciones, archivos, ventanas—
//! aparecen sin prefijo, que es lo que uno quiere casi siempre. Lo que faltaba
//! era el otro caso: **acotar**. «notas» trae la aplicación Notas y tapa el
//! archivo `notas.md`; «terminal» trae la terminal y tapa la ventana de
//! terminal que ya está abierta.
//!
//! Para eso va una letra y un espacio: `f ` archivos, `w ` ventanas, `a `
//! aplicaciones. **Acota, no prioriza**: mostrar el resto abajo devuelve
//! exactamente el problema que se quiso resolver.
//!
//! Y como una letra y un espacio es algo que se escribe sin querer —«a casa»,
//! un archivo llamado «w 3»—, **doblar la letra escapa**: `ww 3` busca el texto
//! «w 3» en todos lados. Es la única salida que no necesita una segunda
//! gramática: no hay comillas, ni un carácter nuevo, ni una tecla que aprender
//! aparte de la que ya se aprendió.
//!
//! # Por qué no hay un rasgo `Proveedor`
//!
//! Porque cada uno recibe algo distinto —las aplicaciones necesitan el catálogo
//! y los pesos de uso, los recientes su lista, el resto nada— y devolver todo
//! por la misma puerta obligaría a pasarles a todos lo que necesita el que más
//! necesita. Con cinco funciones y un `match` se lee de arriba abajo. El rasgo
//! va a tener sentido cuando entren los archivos (#7), que tardan y van a
//! necesitar contestar por partes.

pub mod archivos;
pub mod calculo;
pub mod configuracion;
pub mod emoji;
pub mod expresion;
pub mod moneda;
pub mod recientes;
pub mod shell;
pub mod unidades;
pub mod ventanas;
pub mod web;

use crate::catalogo::aplicacion::Resultado;

/// El piso de los resultados con prefijo.
///
/// Arriba de cualquier puntaje de texto —que llega hasta 100— porque cuando hay
/// prefijo no compiten con nada: es lo único que se muestra, y el número sólo
/// ordena entre ellos.
const PISO: f64 = 1000.0;

pub fn puntaje_de_prefijo(base: f64) -> f64 {
    PISO + base
}

/// Los prefijos que se quedan con la consulta entera.
pub const PREFIJOS: &[char] = &['>', '?', ':'];

/// A cuál de los tres sin prefijo acota una letra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Acotado {
    Aplicaciones,
    Archivos,
    Ventanas,
}

/// Las letras que acotan, con lo que acotan.
///
/// En una tabla y no en el `match` para que la ayuda de la interfaz y la prueba
/// que las recorre no tengan que repetirlas: una letra que se agregue acá
/// aparece sola en las dos.
pub const ACOTAN: &[(char, Acotado)] = &[
    ('a', Acotado::Aplicaciones),
    ('f', Acotado::Archivos),
    ('w', Acotado::Ventanas),
];

/// Cómo hay que repartir lo escrito.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reparto<'a> {
    /// A un solo proveedor, con lo que queda después de la letra.
    Uno(Acotado, &'a str),
    /// A todos, con la consulta **ya desdoblada** si venía escapada.
    Todos(&'a str),
}

/// Reparte lo escrito entre un proveedor y todos.
///
/// La letra sola no alcanza: hace falta el espacio. Sin esa regla, escribir
/// «firefox» empezaría acotando a archivos en la primera tecla y la lista
/// saltaría de un proveedor a otro mientras se escribe.
pub fn repartir(consulta: &str) -> Reparto<'_> {
    let consulta = consulta.trim_start();
    let mut caracteres = consulta.chars();

    let Some(letra) = caracteres.next() else {
        return Reparto::Todos(consulta);
    };
    let minuscula = letra.to_ascii_lowercase();

    let Some((_, adonde)) = ACOTAN.iter().find(|(cual, _)| *cual == minuscula) else {
        return Reparto::Todos(consulta);
    };

    let segunda = caracteres.next();

    // `f algo`: acota.
    if segunda == Some(' ') {
        return Reparto::Uno(*adonde, caracteres.as_str());
    }

    // `ff algo`: no acota, y lo que se busca es `f algo`. Se corta un carácter
    // en vez de armar una cadena nueva: la letra que sobra es la primera, así
    // que lo que queda ya es exactamente el texto que se quiso escribir.
    if segunda.map(|una| una.to_ascii_lowercase()) == Some(minuscula)
        && caracteres.next() == Some(' ')
    {
        return Reparto::Todos(&consulta[letra.len_utf8()..]);
    }

    Reparto::Todos(consulta)
}

/// Contesta si lo escrito arranca con un prefijo.
///
/// `None` significa «esto no es mío»: la consulta sigue su camino normal.
pub fn por_prefijo(consulta: &str, limite: usize) -> Option<Vec<Resultado>> {
    let consulta = consulta.trim_start();
    let mut caracteres = consulta.chars();
    let prefijo = caracteres.next()?;
    let resto = caracteres.as_str();

    match prefijo {
        '>' => Some(shell::buscar(resto)),
        '?' => Some(web::buscar(resto)),
        ':' => Some(emoji::buscar(resto, limite)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_letra_y_un_espacio_acotan() {
        // Los dos casos del issue: «notas» trae la aplicación y tapa el
        // archivo, «terminal» trae la terminal y tapa la ventana abierta.
        assert_eq!(
            repartir("f notas"),
            Reparto::Uno(Acotado::Archivos, "notas")
        );
        assert_eq!(
            repartir("w terminal"),
            Reparto::Uno(Acotado::Ventanas, "terminal")
        );
        assert_eq!(
            repartir("a notas"),
            Reparto::Uno(Acotado::Aplicaciones, "notas")
        );
    }

    #[test]
    fn todas_las_letras_de_la_tabla_acotan() {
        // Recorre la tabla en vez de repetirla: una letra que se agregue queda
        // probada sin tocar esto, que es lo que hace que la tabla valga la pena.
        for (letra, adonde) in ACOTAN {
            let consulta = format!("{letra} algo");
            assert_eq!(
                repartir(&consulta),
                Reparto::Uno(*adonde, "algo"),
                "«{letra} » tendría que acotar"
            );
        }
    }

    #[test]
    fn la_letra_sola_no_acota() {
        // Sin esta regla, escribir «firefox» acotaría a archivos en la primera
        // tecla y la lista saltaría de un proveedor a otro mientras se escribe.
        assert_eq!(repartir("firefox"), Reparto::Todos("firefox"));
        assert_eq!(repartir("f"), Reparto::Todos("f"));
        assert_eq!(repartir("wayland"), Reparto::Todos("wayland"));
    }

    #[test]
    fn doblar_la_letra_escapa() {
        // El caso que el issue dejaba abierto: un archivo que se llama «w 3».
        assert_eq!(repartir("ww 3"), Reparto::Todos("w 3"));
        assert_eq!(repartir("ff notas"), Reparto::Todos("f notas"));
        assert_eq!(repartir("aa casa"), Reparto::Todos("a casa"));
    }

    #[test]
    fn doblar_una_letra_que_no_acota_no_escapa_nada() {
        // «zz algo» es texto, no un escape: si se desdoblara, buscar un archivo
        // llamado «zz» sería imposible y nadie sabría por qué.
        assert_eq!(repartir("zz algo"), Reparto::Todos("zz algo"));
        assert_eq!(repartir("ffmpeg"), Reparto::Todos("ffmpeg"));
    }

    #[test]
    fn una_letra_que_no_esta_en_la_tabla_no_acota() {
        assert_eq!(repartir("x algo"), Reparto::Todos("x algo"));
        assert_eq!(repartir("z algo"), Reparto::Todos("z algo"));
    }

    #[test]
    fn se_acota_con_mayuscula_tambien() {
        // Quien deja el bloqueo de mayúsculas puesto no está pidiendo otra cosa.
        assert_eq!(
            repartir("F notas"),
            Reparto::Uno(Acotado::Archivos, "notas")
        );
        assert_eq!(repartir("FF notas"), Reparto::Todos("F notas"));
    }

    #[test]
    fn los_espacios_de_adelante_no_cambian_nada() {
        assert_eq!(
            repartir("   f notas"),
            Reparto::Uno(Acotado::Archivos, "notas")
        );
        assert_eq!(repartir("  ww 3"), Reparto::Todos("w 3"));
    }

    #[test]
    fn acotar_sin_escribir_nada_acota_igual() {
        // `f ` y nada más no devuelve filas, pero acota: es lo que hace que la
        // interfaz pueda decir en qué modo está antes de que se escriba.
        assert_eq!(repartir("f "), Reparto::Uno(Acotado::Archivos, ""));
    }

    #[test]
    fn la_consulta_vacia_no_rompe() {
        assert_eq!(repartir(""), Reparto::Todos(""));
        assert_eq!(repartir("   "), Reparto::Todos(""));
    }

    #[test]
    fn un_primer_caracter_de_varios_bytes_no_rompe() {
        // `repartir` corta la cadena por bytes para desdoblar. Con un carácter
        // multibyte adelante, cortar en 1 sería un pánico por índice que no cae
        // en un límite de carácter.
        assert_eq!(repartir("ñ algo"), Reparto::Todos("ñ algo"));
        assert_eq!(repartir("日 本"), Reparto::Todos("日 本"));
        assert_eq!(repartir("é"), Reparto::Todos("é"));
    }

    #[test]
    fn los_prefijos_de_un_caracter_no_los_toca() {
        // `> f algo` es un comando que empieza con «f». El reparto por letra
        // corre después del de un carácter, y acá se fija que no se adelante.
        assert_eq!(repartir("> f algo"), Reparto::Todos("> f algo"));
        assert_eq!(repartir("? f algo"), Reparto::Todos("? f algo"));
    }
}

/// Que la ayuda de la ventana nombre todos los prefijos que existen.
///
/// La ayuda vive en los catálogos de idioma y las letras en la tabla de acá:
/// son dos listas, y las dos listas se separan. Agregar una letra y olvidarse
/// de la ayuda no rompe nada —el prefijo anda igual— y por eso no se nota: sólo
/// queda uno que nadie va a descubrir nunca, que es lo mismo que no tenerlo.
#[cfg(test)]
mod ayuda {
    use super::*;

    fn catalogo(idioma: &str) -> String {
        let ruta = format!("locales/{idioma}.yml");
        std::fs::read_to_string(&ruta).unwrap_or_else(|_| panic!("falta {ruta}"))
    }

    fn linea_de_la_ayuda(idioma: &str) -> String {
        catalogo(idioma)
            .lines()
            .find(|linea| linea.trim_start().starts_with("prefijos:"))
            .unwrap_or_else(|| panic!("falta `lanzador.prefijos` en {idioma}.yml"))
            .to_string()
    }

    #[test]
    fn la_ayuda_nombra_todas_las_letras_que_acotan() {
        for idioma in ["es", "en"] {
            let ayuda = linea_de_la_ayuda(idioma);

            for (letra, adonde) in ACOTAN {
                assert!(
                    ayuda.contains(&format!("{letra} ")),
                    "{idioma}.yml no nombra «{letra} » ({adonde:?}): es un prefijo que nadie va a descubrir"
                );
            }
        }
    }

    #[test]
    fn y_tambien_los_de_un_caracter() {
        for idioma in ["es", "en"] {
            let ayuda = linea_de_la_ayuda(idioma);

            for prefijo in PREFIJOS {
                assert!(
                    ayuda.contains(*prefijo),
                    "{idioma}.yml no nombra «{prefijo}»"
                );
            }
        }
    }
}
