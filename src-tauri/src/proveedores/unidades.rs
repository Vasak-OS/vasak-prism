//! Convertir entre unidades.
//!
//! «3 pulgadas a cm», «80 kg en libras», «2 horas a minutos». Lo que se busca
//! es lo que uno escribiría en el cuadro de un buscador, no una notación: por
//! eso los nombres van en los dos idiomas y en singular y plural, y el enlace
//! puede ser «a», «en», «to» o «in».
//!
//! Todo se pasa por una unidad base por familia y se vuelve. La temperatura no
//! entra en ese molde —no es una escala proporcional— así que va aparte.

/// Las familias. Dos unidades sólo se convierten entre sí dentro de una.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Familia {
    Longitud,
    Masa,
    Tiempo,
    Datos,
    Temperatura,
}

struct Unidad {
    /// Cómo se la puede escribir. El primero es el que se muestra.
    nombres: &'static [&'static str],
    familia: Familia,
    /// Cuántas unidades base es una de éstas.
    en_base: f64,
}

/// Metro, gramo, segundo y byte.
const UNIDADES: &[Unidad] = &[
    // Longitud, base el metro.
    Unidad {
        nombres: &["mm", "milimetro", "milimetros", "milímetro", "milímetros"],
        familia: Familia::Longitud,
        en_base: 0.001,
    },
    Unidad {
        nombres: &[
            "cm",
            "centimetro",
            "centimetros",
            "centímetro",
            "centímetros",
        ],
        familia: Familia::Longitud,
        en_base: 0.01,
    },
    Unidad {
        nombres: &["m", "metro", "metros", "meter", "meters"],
        familia: Familia::Longitud,
        en_base: 1.0,
    },
    Unidad {
        nombres: &["km", "kilometro", "kilometros", "kilómetro", "kilómetros"],
        familia: Familia::Longitud,
        en_base: 1000.0,
    },
    Unidad {
        nombres: &["in", "pulgada", "pulgadas", "inch", "inches"],
        familia: Familia::Longitud,
        en_base: 0.0254,
    },
    Unidad {
        nombres: &["ft", "pie", "pies", "foot", "feet"],
        familia: Familia::Longitud,
        en_base: 0.3048,
    },
    Unidad {
        nombres: &["mi", "milla", "millas", "mile", "miles"],
        familia: Familia::Longitud,
        en_base: 1609.344,
    },
    // Masa, base el gramo.
    Unidad {
        nombres: &["mg", "miligramo", "miligramos"],
        familia: Familia::Masa,
        en_base: 0.001,
    },
    Unidad {
        nombres: &["g", "gramo", "gramos", "gram", "grams"],
        familia: Familia::Masa,
        en_base: 1.0,
    },
    Unidad {
        nombres: &["kg", "kilo", "kilos", "kilogramo", "kilogramos"],
        familia: Familia::Masa,
        en_base: 1000.0,
    },
    Unidad {
        nombres: &["t", "tonelada", "toneladas", "ton", "tons"],
        familia: Familia::Masa,
        en_base: 1_000_000.0,
    },
    Unidad {
        nombres: &["oz", "onza", "onzas", "ounce", "ounces"],
        familia: Familia::Masa,
        en_base: 28.349523125,
    },
    Unidad {
        nombres: &["lb", "libra", "libras", "pound", "pounds"],
        familia: Familia::Masa,
        en_base: 453.59237,
    },
    // Tiempo, base el segundo.
    Unidad {
        nombres: &["s", "seg", "segundo", "segundos", "second", "seconds"],
        familia: Familia::Tiempo,
        en_base: 1.0,
    },
    Unidad {
        nombres: &["min", "minuto", "minutos", "minute", "minutes"],
        familia: Familia::Tiempo,
        en_base: 60.0,
    },
    Unidad {
        nombres: &["h", "hora", "horas", "hour", "hours"],
        familia: Familia::Tiempo,
        en_base: 3600.0,
    },
    Unidad {
        nombres: &["d", "dia", "dias", "día", "días", "day", "days"],
        familia: Familia::Tiempo,
        en_base: 86400.0,
    },
    // Datos, base el byte. Los de mil y los de 1024, que no son lo mismo y por
    // eso están los dos: un disco de 1 TB tiene 0,909 TiB, y ésa es justamente
    // la cuenta que uno viene a hacer acá.
    Unidad {
        nombres: &["b", "byte", "bytes"],
        familia: Familia::Datos,
        en_base: 1.0,
    },
    Unidad {
        nombres: &["kb", "kilobyte", "kilobytes"],
        familia: Familia::Datos,
        en_base: 1_000.0,
    },
    Unidad {
        nombres: &["mb", "megabyte", "megabytes"],
        familia: Familia::Datos,
        en_base: 1_000_000.0,
    },
    Unidad {
        nombres: &["gb", "gigabyte", "gigabytes"],
        familia: Familia::Datos,
        en_base: 1_000_000_000.0,
    },
    Unidad {
        nombres: &["tb", "terabyte", "terabytes"],
        familia: Familia::Datos,
        en_base: 1_000_000_000_000.0,
    },
    Unidad {
        nombres: &["kib"],
        familia: Familia::Datos,
        en_base: 1024.0,
    },
    Unidad {
        nombres: &["mib"],
        familia: Familia::Datos,
        en_base: 1_048_576.0,
    },
    Unidad {
        nombres: &["gib"],
        familia: Familia::Datos,
        en_base: 1_073_741_824.0,
    },
    Unidad {
        nombres: &["tib"],
        familia: Familia::Datos,
        en_base: 1_099_511_627_776.0,
    },
    // Temperatura: los factores no se usan, la conversión va aparte.
    Unidad {
        nombres: &["c", "celsius", "centigrados", "centígrados"],
        familia: Familia::Temperatura,
        en_base: 1.0,
    },
    Unidad {
        nombres: &["f", "fahrenheit"],
        familia: Familia::Temperatura,
        en_base: 1.0,
    },
    Unidad {
        nombres: &["k", "kelvin", "kelvins"],
        familia: Familia::Temperatura,
        en_base: 1.0,
    },
];

/// Las palabras que unen las dos unidades: «3 pulgadas **a** cm».
const ENLACES: &[&str] = &["a", "en", "to", "in"];

fn buscar(nombre: &str) -> Option<&'static Unidad> {
    let nombre = nombre.trim().to_lowercase();
    UNIDADES
        .iter()
        .find(|unidad| unidad.nombres.iter().any(|uno| *uno == nombre))
}

/// Una conversión resuelta.
pub struct Conversion {
    pub valor: f64,
    /// El nombre corto de la unidad de destino, para mostrarlo al lado.
    pub unidad: &'static str,
}

/// Lee «3 pulgadas a cm» y devuelve el resultado.
///
/// `None` en cuanto algo no encaja: no es una conversión y hay que dejar que la
/// consulta siga su camino.
pub fn convertir(consulta: &str) -> Option<Conversion> {
    let palabras: Vec<&str> = consulta.split_whitespace().collect();
    if palabras.len() < 3 {
        return None;
    }

    // El enlace parte la consulta en dos. Se busca desde el final: «in» es a la
    // vez un enlace y la pulgada, y en «3 in in cm» el que une es el segundo.
    let corte = palabras
        .iter()
        .rposition(|palabra| ENLACES.contains(&palabra.to_lowercase().as_str()))?;

    let izquierda = &palabras[..corte];
    let derecha = &palabras[corte + 1..];

    if izquierda.len() < 2 || derecha.len() != 1 {
        return None;
    }

    let origen = buscar(izquierda[izquierda.len() - 1])?;
    let destino = buscar(derecha[0])?;

    if origen.familia != destino.familia {
        return None;
    }

    // Lo de adelante de la unidad tiene que ser un número y nada más: en «tres
    // metros a cm» no hay número, y en «abrir 3 metros a cm» sobra una palabra.
    let cantidad = izquierda[..izquierda.len() - 1].join("");
    let cantidad: f64 = cantidad.replace(',', ".").parse().ok()?;

    let valor = if origen.familia == Familia::Temperatura {
        temperatura(cantidad, origen.nombres[0], destino.nombres[0])?
    } else {
        cantidad * origen.en_base / destino.en_base
    };

    valor.is_finite().then_some(Conversion {
        valor,
        unidad: destino.nombres[0],
    })
}

/// La temperatura no es proporcional: cero grados no es cero de nada.
fn temperatura(valor: f64, de: &str, a: &str) -> Option<f64> {
    let celsius = match de {
        "c" => valor,
        "f" => (valor - 32.0) * 5.0 / 9.0,
        "k" => valor - 273.15,
        _ => return None,
    };

    Some(match a {
        "c" => celsius,
        "f" => celsius * 9.0 / 5.0 + 32.0,
        "k" => celsius + 273.15,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proveedores::expresion::formatear;

    fn convertida(consulta: &str) -> Option<String> {
        convertir(consulta).map(|c| format!("{} {}", formatear(c.valor), c.unidad))
    }

    #[test]
    fn la_longitud_se_convierte() {
        assert_eq!(convertida("3 pulgadas a cm"), Some("7.62 cm".to_string()));
        assert_eq!(convertida("1 km a m"), Some("1000 m".to_string()));
    }

    #[test]
    fn la_masa_tambien() {
        assert_eq!(convertida("1 kg a g"), Some("1000 g".to_string()));
        assert_eq!(convertida("2 lb a kg"), Some("0.90718474 kg".to_string()));
    }

    #[test]
    fn el_tiempo_tambien() {
        assert_eq!(convertida("2 horas a minutos"), Some("120 min".to_string()));
        assert_eq!(convertida("90 min en h"), Some("1.5 h".to_string()));
    }

    #[test]
    fn los_de_mil_y_los_de_1024_no_son_lo_mismo() {
        // Es justo la cuenta que uno viene a hacer: un disco de 1 TB tiene
        // 0,909 TiB, y por eso el sistema «miente».
        assert_eq!(
            convertida("1 tb a tib"),
            Some("0.909494701773 tib".to_string())
        );
        assert_eq!(convertida("1 gib a mb"), Some("1073.741824 mb".to_string()));
    }

    #[test]
    fn la_temperatura_no_es_proporcional() {
        // Cero grados no es cero de nada: con una regla de tres da cualquier
        // cosa.
        assert_eq!(convertida("0 c a f"), Some("32 f".to_string()));
        assert_eq!(convertida("100 c a f"), Some("212 f".to_string()));
        assert_eq!(convertida("98.6 f a c"), Some("37 c".to_string()));
        assert_eq!(convertida("0 c a k"), Some("273.15 k".to_string()));
    }

    #[test]
    fn el_enlace_va_en_los_dos_idiomas() {
        assert!(convertida("1 m a cm").is_some());
        assert!(convertida("1 m en cm").is_some());
        assert!(convertida("1 m to cm").is_some());
        assert!(convertida("1 m in cm").is_some());
    }

    #[test]
    fn la_pulgada_y_el_enlace_se_escriben_igual() {
        // `in` es la unidad y también la palabra que une. El que une es el
        // último, y sin eso «3 in in cm» no se entiende.
        assert_eq!(convertida("3 in in cm"), Some("7.62 cm".to_string()));
    }

    #[test]
    fn la_coma_decimal_se_entiende() {
        assert_eq!(convertida("2,5 m a cm"), Some("250 cm".to_string()));
    }

    #[test]
    fn dos_familias_distintas_no_se_convierten() {
        // Un kilo de metros no existe, y ofrecer un número sería peor que no
        // ofrecer nada.
        assert_eq!(convertida("1 kg a m"), None);
        assert_eq!(convertida("3 horas a cm"), None);
    }

    #[test]
    fn lo_que_no_es_una_conversion_no_da_una() {
        assert_eq!(convertida("firefox"), None);
        assert_eq!(convertida("editor de texto"), None);
        assert_eq!(convertida("ir a casa"), None);
        assert_eq!(convertida("tres metros a cm"), None);
        assert_eq!(convertida("1 m a"), None);
    }

    #[test]
    fn una_unidad_que_no_existe_no_da_resultado() {
        assert_eq!(convertida("3 bananas a cm"), None);
    }
}
