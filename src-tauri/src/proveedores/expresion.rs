//! Evaluar una expresión aritmética.
//!
//! Escrito a mano y no con una biblioteca porque lo que hace falta es chico y
//! muy concreto —lo que alguien escribe en un lanzador, no un lenguaje— y
//! porque la mitad del trabajo es decidir qué **no** es una cuenta: escribir
//! «firefox» no puede dar un resultado de calculadora arriba de todo.
//!
//! Se acepta la coma como separador decimal, que es como se escribe acá, y la
//! respuesta sale con punto: es la que se copia al portapapeles y de ahí suele
//! ir a parar a un campo o a un archivo, donde la coma no se entiende.

/// Cuánto se redondea antes de mostrar.
///
/// Doce dígitos significativos: suficiente para que `0.1 + 0.2` dé `0.3` y no
/// `0.30000000000000004`, y corto de sobra para no perder nada de lo que se
/// escribe en un lanzador.
const DIGITOS: i32 = 12;

#[derive(Debug, Clone, PartialEq)]
enum Pieza {
    Numero(f64),
    Mas,
    Menos,
    Por,
    Dividido,
    Potencia,
    Abre,
    Cierra,
    /// El `%` pegado a un número: `15%` es 0,15 cuando acompaña y 15 cuando
    /// está solo. Lo resuelve quien arma la cuenta.
    Porciento,
}

/// Parte el texto en piezas. `None` si aparece algo que no es de una cuenta.
fn trocear(texto: &str) -> Option<Vec<Pieza>> {
    let mut piezas = Vec::new();
    let caracteres: Vec<char> = texto.chars().collect();
    let mut i = 0;

    while i < caracteres.len() {
        let c = caracteres[i];

        if c.is_whitespace() {
            i += 1;
            continue;
        }

        if c.is_ascii_digit() || c == '.' || c == ',' {
            let (numero, avance) = leer_numero(&caracteres[i..])?;
            piezas.push(Pieza::Numero(numero));
            i += avance;
            continue;
        }

        piezas.push(match c {
            '+' => Pieza::Mas,
            '-' | '−' => Pieza::Menos,
            '*' | '×' | 'x' | 'X' => Pieza::Por,
            '/' | '÷' => Pieza::Dividido,
            '%' => Pieza::Porciento,
            '^' => Pieza::Potencia,
            '(' => Pieza::Abre,
            ')' => Pieza::Cierra,
            // Cualquier otra cosa y esto no es una cuenta. Callar y seguir
            // haría que «firefox» evaluara a algo.
            _ => return None,
        });
        i += 1;
    }

    (!piezas.is_empty()).then_some(piezas)
}

/// Lee un número desde el principio: decimal, hexadecimal o binario.
fn leer_numero(desde: &[char]) -> Option<(f64, usize)> {
    // `0x1f` y `0b1010`, que es como se escriben cuando uno está mirando código.
    if desde.len() > 2 && desde[0] == '0' {
        let base = match desde[1] {
            'x' | 'X' => Some(16),
            'b' | 'B' => Some(2),
            _ => None,
        };

        if let Some(base) = base {
            let digitos: String = desde[2..]
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric())
                .collect();
            if let Ok(valor) = i64::from_str_radix(&digitos, base) {
                return Some((valor as f64, 2 + digitos.len()));
            }
            return None;
        }
    }

    let mut texto = String::new();
    let mut usados = 0;
    let mut hay_separador = false;

    for c in desde {
        match c {
            c if c.is_ascii_digit() => texto.push(*c),
            // Un solo separador decimal, y la coma vale igual que el punto: acá
            // se escribe `2,5` mucho más seguido que `2.5`.
            '.' | ',' if !hay_separador => {
                hay_separador = true;
                texto.push('.');
            }
            _ => break,
        }
        usados += 1;
    }

    texto.parse().ok().map(|valor| (valor, usados))
}

/// Evalúa la lista de piezas. Descenso recursivo, de menor a mayor precedencia.
struct Cuenta {
    piezas: Vec<Pieza>,
    donde: usize,
}

impl Cuenta {
    fn mirar(&self) -> Option<&Pieza> {
        self.piezas.get(self.donde)
    }

    fn sumas(&mut self) -> Option<f64> {
        let mut valor = self.productos()?;

        loop {
            match self.mirar() {
                Some(Pieza::Mas) => {
                    self.donde += 1;
                    let otro = self.productos()?;
                    // `340 + 15%` es 340 más el 15 **de 340**, que es lo que
                    // uno quiere decir. Con el porcentaje suelto sería 340,15.
                    valor += if self.venia_un_porciento() {
                        valor * otro / 100.0
                    } else {
                        otro
                    };
                }
                Some(Pieza::Menos) => {
                    self.donde += 1;
                    let otro = self.productos()?;
                    valor -= if self.venia_un_porciento() {
                        valor * otro / 100.0
                    } else {
                        otro
                    };
                }
                _ => return Some(valor),
            }
        }
    }

    /// Si la pieza recién consumida era un porcentaje.
    fn venia_un_porciento(&self) -> bool {
        self.donde
            .checked_sub(1)
            .and_then(|antes| self.piezas.get(antes))
            == Some(&Pieza::Porciento)
    }

    fn productos(&mut self) -> Option<f64> {
        let mut valor = self.potencias()?;

        loop {
            let dividir = match self.mirar() {
                Some(Pieza::Por) => false,
                Some(Pieza::Dividido) => true,
                _ => return Some(valor),
            };
            self.donde += 1;
            let otro = self.potencias()?;

            if dividir {
                // Dividir por cero da infinito en coma flotante, y «inf» no es
                // una respuesta: mejor no ofrecer ninguna.
                if otro == 0.0 {
                    return None;
                }
                valor /= otro;
            } else {
                valor *= otro;
            }
        }
    }

    fn potencias(&mut self) -> Option<f64> {
        let base = self.unario()?;

        if self.mirar() == Some(&Pieza::Potencia) {
            self.donde += 1;
            // A la derecha, que es como se asocia: `2^3^2` es 2^9.
            let exponente = self.potencias()?;
            return Some(base.powf(exponente));
        }

        Some(base)
    }

    fn unario(&mut self) -> Option<f64> {
        match self.mirar() {
            Some(Pieza::Menos) => {
                self.donde += 1;
                Some(-self.unario()?)
            }
            Some(Pieza::Mas) => {
                self.donde += 1;
                self.unario()
            }
            _ => self.atomo(),
        }
    }

    fn atomo(&mut self) -> Option<f64> {
        match self.mirar().cloned() {
            Some(Pieza::Numero(valor)) => {
                self.donde += 1;
                // Un `%` detrás del número se consume acá; qué significa lo
                // decide quien lo recibe.
                if self.mirar() == Some(&Pieza::Porciento) {
                    self.donde += 1;
                }
                Some(valor)
            }
            Some(Pieza::Abre) => {
                self.donde += 1;
                let valor = self.sumas()?;
                if self.mirar() != Some(&Pieza::Cierra) {
                    return None;
                }
                self.donde += 1;
                Some(valor)
            }
            _ => None,
        }
    }
}

/// El resultado de la expresión, o `None` si no es una.
pub fn evaluar(texto: &str) -> Option<f64> {
    let piezas = trocear(texto)?;

    // Un número solo no es una cuenta: escribir «42» tiene que devolver lo que
    // se llame 42, no ofrecer «42» como resultado de calcular 42.
    if piezas.len() == 1 && matches!(piezas[0], Pieza::Numero(_)) {
        return None;
    }

    let mut cuenta = Cuenta { piezas, donde: 0 };
    let valor = cuenta.sumas()?;

    // Si sobró algo, la expresión estaba mal: `2 + + 3` no es 5.
    if cuenta.donde != cuenta.piezas.len() {
        return None;
    }

    valor.is_finite().then_some(valor)
}

/// El número como se muestra y como se copia.
pub fn formatear(valor: f64) -> String {
    let redondeado = redondear(valor);

    if redondeado == redondeado.trunc() && redondeado.abs() < 1e15 {
        return format!("{}", redondeado as i64);
    }

    // Sin ceros de relleno al final: `2.50` se lee peor que `2.5`.
    let texto = format!("{redondeado:.*}", DIGITOS as usize);
    texto
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// Redondea a los dígitos significativos que se muestran.
fn redondear(valor: f64) -> f64 {
    if valor == 0.0 || !valor.is_finite() {
        return valor;
    }

    let magnitud = valor.abs().log10().floor() as i32;
    let factor = 10f64.powi(DIGITOS - 1 - magnitud);
    (valor * factor).round() / factor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_cuatro_operaciones() {
        assert_eq!(evaluar("2+2"), Some(4.0));
        assert_eq!(evaluar("10 - 3"), Some(7.0));
        assert_eq!(evaluar("6 * 7"), Some(42.0));
        assert_eq!(evaluar("10 / 4"), Some(2.5));
    }

    #[test]
    fn la_precedencia_es_la_de_siempre() {
        assert_eq!(evaluar("2 + 3 * 4"), Some(14.0));
        assert_eq!(evaluar("(2 + 3) * 4"), Some(20.0));
        assert_eq!(evaluar("2 ^ 3 ^ 2"), Some(512.0));
    }

    #[test]
    fn el_menos_de_adelante_es_un_signo() {
        assert_eq!(evaluar("-5 + 3"), Some(-2.0));
        assert_eq!(evaluar("3 * -2"), Some(-6.0));
    }

    #[test]
    fn la_coma_decimal_se_entiende() {
        // Es como se escribe acá, y quien escribe `2,5` no está separando dos
        // números.
        assert_eq!(evaluar("2,5 * 2"), Some(5.0));
        assert_eq!(evaluar("2.5 * 2"), Some(5.0));
    }

    #[test]
    fn el_por_se_puede_escribir_con_equis() {
        // Es lo que sale del teclado sin buscar el asterisco.
        assert_eq!(evaluar("6 x 7"), Some(42.0));
    }

    #[test]
    fn el_porcentaje_de_una_suma_es_sobre_el_total() {
        // `340 + 15%` es 340 más el 15 **de 340**. Sumar 0,15 sería la otra
        // lectura, y no es la que nadie quiere decir.
        assert_eq!(evaluar("340 + 15%"), Some(391.0));
        assert_eq!(evaluar("340 - 15%"), Some(289.0));
    }

    #[test]
    fn los_literales_de_código_se_entienden() {
        assert_eq!(evaluar("0xff + 1"), Some(256.0));
        assert_eq!(evaluar("0b1010 * 2"), Some(20.0));
    }

    #[test]
    fn dividir_por_cero_no_da_respuesta() {
        // «inf» no es un resultado que sirva de nada arriba de la lista.
        assert_eq!(evaluar("1/0"), None);
        assert_eq!(evaluar("(3 - 3) / (1 - 1)"), None);
    }

    #[test]
    fn un_numero_solo_no_es_una_cuenta() {
        // Escribir «42» tiene que buscar lo que se llame 42, no ofrecer 42 como
        // resultado de calcular 42.
        assert_eq!(evaluar("42"), None);
        assert_eq!(evaluar("  7  "), None);
    }

    #[test]
    fn lo_que_no_es_una_cuenta_no_da_una() {
        // La mitad del trabajo: que escribir el nombre de un programa no ponga
        // una calculadora arriba de todo.
        assert_eq!(evaluar("firefox"), None);
        assert_eq!(evaluar("editor de texto"), None);
        assert_eq!(evaluar(""), None);
        assert_eq!(evaluar("+"), None);
    }

    #[test]
    fn una_expresion_incompleta_no_da_resultado() {
        assert_eq!(evaluar("2 +"), None);
        assert_eq!(evaluar("2 + * 3"), None);
        assert_eq!(evaluar("(2 + 3"), None);
        assert_eq!(evaluar("2 + 3)"), None);
    }

    #[test]
    fn el_mas_de_adelante_tambien_es_un_signo() {
        // `2 + +3` es 5 y no un error: si el menos de adelante vale, el más
        // también. Sale gratis del mismo camino y no hay motivo para rechazarlo.
        assert_eq!(evaluar("2 + +3"), Some(5.0));
        assert_eq!(evaluar("+5 * 2"), Some(10.0));
    }

    #[test]
    fn el_resultado_no_arrastra_la_basura_de_la_coma_flotante() {
        assert_eq!(formatear(evaluar("0,1 + 0,2").unwrap()), "0.3");
        assert_eq!(formatear(evaluar("1 / 3").unwrap()), "0.333333333333");
    }

    #[test]
    fn un_entero_se_muestra_sin_coma() {
        assert_eq!(formatear(4.0), "4");
        assert_eq!(formatear(-7.0), "-7");
        assert_eq!(formatear(0.0), "0");
    }

    #[test]
    fn la_respuesta_sale_con_punto_y_no_con_coma() {
        // Se copia al portapapeles y de ahí suele ir a un campo o a un archivo,
        // donde la coma no se entiende.
        assert_eq!(formatear(2.5), "2.5");
    }
}
