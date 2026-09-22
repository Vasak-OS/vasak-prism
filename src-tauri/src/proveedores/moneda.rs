//! Convertir monedas: «100 usd a pesos».
//!
//! Sale del proveedor de cálculo, que se quedó con las unidades y sin la
//! moneda. La diferencia es que una pulgada mide lo mismo todos los días y un
//! dólar no, así que esto es lo único de Prism que necesita la red.
//!
//! # Lo que nunca pasa en el camino de la consulta
//!
//! **Escribir no espera nada.** La conversión se resuelve contra una tabla que
//! ya está en memoria; si no hay tabla, no hay fila y la consulta sigue su
//! camino. Nunca se abre una conexión mientras alguien escribe: un lanzador que
//! consulta una API en cada tecla es un lanzador que no se puede usar.
//!
//! **Y no se toca la red hasta que alguien pide una conversión.** El daemon
//! arranca leyendo lo que haya en disco y nada más. La primera consulta de
//! moneda es la que autoriza, de hecho, el primer pedido: quien nunca convierte
//! una moneda nunca hace que Prism hable con afuera. Es una propiedad del
//! diseño y no una opción que haya que acordarse de apagar.
//!
//! # Sin red
//!
//! Se contesta con lo guardado **diciendo de cuándo es**. Una cotización vieja
//! sin fecha es peor que ninguna: quien la ve la toma por la de hoy. La fila
//! lleva la fecha en el subtítulo siempre, tenga un día o tenga tres semanas.
//!
//! # Dónde se guarda
//!
//! En `$XDG_DATA_HOME` y no en la caché, que es la excepción del sistema y
//! merece el motivo escrito. El criterio del resto es «si se puede rehacer, va
//! a caché»; acá rehacer necesita red, y el momento en que la caché se borra es
//! exactamente el momento en que puede no haberla. Es la misma razón por la que
//! la frecuencia de uso vive en datos: no es que sea valiosa, es que no hay de
//! dónde sacarla de nuevo.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::catalogo::aplicacion::{Origen, Resultado};

use super::expresion;

/// Dónde se guarda lo último que se trajo, relativo al directorio de datos.
const ARCHIVO: &str = "vasak-prism/cotizaciones.json";

/// De dónde salen las cotizaciones.
///
/// `@fawazahmed0/currency-api`, en dominio público (CC0) y sin clave ni
/// registro — que era la condición: una clave no se puede meter en un paquete
/// que instala cualquiera. Son archivos estáticos en un CDN, así que el modo de
/// fallo no es «el servicio se cayó» sino «los archivos dejan de actualizarse»,
/// que con la fecha a la vista se nota.
///
/// Las dos direcciones son la primaria y el respaldo que la propia fuente pide
/// implementar. Sin el segundo, una caída del CDN es el proveedor entero caído.
const FUENTES: &[&str] = &[
    "https://cdn.jsdelivr.net/npm/@fawazahmed0/currency-api@latest/v1/currencies/usd.json",
    "https://latest.currency-api.pages.dev/v1/currencies/usd.json",
];

/// Cuánto espera un pedido antes de darse por perdido.
///
/// Corto a propósito: esto corre en un hilo aparte y nadie lo está esperando,
/// pero un pedido colgado deja el hilo tomado y el próximo intento no arranca.
const ESPERA: std::time::Duration = std::time::Duration::from_secs(15);

/// El icono, por nombre del tema como todos los demás.
const ICONO: &str = "accessories-calculator";

/// Igual que una cuenta: es la respuesta exacta a lo que se escribió.
const PUNTAJE: f64 = 1000.0;

/// Las palabras que unen «100 usd **a** eur».
///
/// Propias y no las de `unidades`: ahí «in» es a la vez enlace y pulgada, y acá
/// no hay unidad que se llame así. Lo que sí hace falta es «=», que en una
/// conversión de moneda se escribe seguido.
const ENLACES: &[&str] = &["a", "to", "en", "in", "=", "->"];

/// Lo que se trajo, tal como se guarda en el disco.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Cotizaciones {
    /// La fecha que publica la fuente, no la de cuándo se bajó: es la que
    /// importa para decir de cuándo es la cotización.
    pub fecha: String,
    /// Cuántas unidades de cada moneda vale un dólar.
    pub por_dolar: HashMap<String, f64>,
    /// Cuándo se trajo, en segundos desde la época.
    ///
    /// No es lo mismo que `fecha`: aquélla es la que publica la fuente —que
    /// suele ir un día atrasada— y ésta es cuándo la bajamos nosotros. La
    /// primera es la que se le muestra a quien pregunta; ésta es la que decide
    /// si conviene volver a pedir, y por eso tiene que sobrevivir al reinicio:
    /// si no, cada arranque volvería a pedir.
    ///
    /// Con `default` para que un archivo escrito antes de este campo se siga
    /// leyendo — se trata como «hace mucho» y se refresca una vez.
    #[serde(default)]
    pub traido_en: u64,
}

impl Cotizaciones {
    /// Cuánto vale `cantidad` de `origen` expresado en `destino`.
    ///
    /// Todo pasa por el dólar porque la fuente publica una tabla por moneda
    /// base y se baja una sola: con las dos contra el dólar alcanza, y es un
    /// pedido en vez de doscientos.
    pub fn convertir(&self, cantidad: f64, origen: &str, destino: &str) -> Option<f64> {
        let de = self.por_dolar.get(origen)?;
        let a = self.por_dolar.get(destino)?;
        // Una tasa en cero llegaría a dividir por cero y dar infinito, que se
        // mostraría como una conversión perfectamente normal.
        (*de != 0.0 && de.is_finite() && a.is_finite())
            .then(|| cantidad / de * a)
            .filter(|valor| valor.is_finite())
    }
}

/// Una conversión resuelta.
#[derive(Debug, PartialEq)]
pub struct Conversion {
    pub valor: f64,
    /// En mayúsculas, como se escriben los códigos.
    pub destino: String,
    /// La fecha de la cotización usada.
    pub fecha: String,
}

/// Cómo se escribe cada moneda además de su código.
///
/// A mano y corta a propósito. La fuente trae trescientas cuarenta y un
/// monedas, y aceptar el nombre largo de todas haría que «real», «won» o «rand»
/// —que son palabras— se leyeran como conversiones. Sólo entran los nombres que
/// en esta región se escriben más que el código.
///
/// «pesos» es ARS porque este sistema se usa acá. No es una verdad universal y
/// por eso está escrito: quien quiera los chilenos escribe `clp`.
const NOMBRES: &[(&str, &str)] = &[
    ("dolar", "usd"),
    ("dolares", "usd"),
    ("dólar", "usd"),
    ("dólares", "usd"),
    ("euro", "eur"),
    ("euros", "eur"),
    ("peso", "ars"),
    ("pesos", "ars"),
    ("real", "brl"),
    ("reales", "brl"),
    ("libra", "gbp"),
    ("libras", "gbp"),
    ("yen", "jpy"),
    ("yenes", "jpy"),
];

/// El código de una moneda, o nada si eso no es una moneda.
fn codigo(palabra: &str) -> Option<String> {
    let limpia = palabra.trim().trim_matches(|c: char| !c.is_alphanumeric());
    if limpia.is_empty() {
        return None;
    }
    let minuscula = limpia.to_lowercase();

    if let Some((_, cod)) = NOMBRES.iter().find(|(nombre, _)| *nombre == minuscula) {
        return Some((*cod).to_string());
    }

    // Un código es de tres letras. No se comprueba contra la tabla acá: eso lo
    // hace `convertir`, que es quien la tiene, y así un código que la fuente
    // deje de publicar se comporta como «no es una conversión» en vez de como
    // un error.
    (minuscula.len() == 3 && minuscula.chars().all(|c| c.is_ascii_alphabetic()))
        .then_some(minuscula)
}

/// Lee «100 usd a eur» y devuelve el resultado.
///
/// `None` en cuanto algo no encaja: no es una conversión de moneda y hay que
/// dejar que la consulta siga su camino.
pub fn convertir(consulta: &str, tabla: &Cotizaciones) -> Option<Conversion> {
    let (cantidad, origen, destino) = partes(consulta)?;
    let valor = tabla.convertir(cantidad, &origen, &destino)?;

    Some(Conversion {
        valor,
        destino: destino.to_uppercase(),
        fecha: tabla.fecha.clone(),
    })
}

/// Si lo escrito **tiene la forma** de una conversión de moneda.
///
/// Sin mirar ninguna tabla, y por eso aparte: es lo que permite saber que
/// alguien está pidiendo una conversión **antes** de tener cotizaciones, que es
/// el momento en que hay que ir a buscarlas. Sin esto habría que pedirlas al
/// arrancar por las dudas, o no tenerlas nunca la primera vez.
pub fn parece_conversion(consulta: &str) -> bool {
    partes(consulta.trim()).is_some()
}

/// La cantidad y los dos códigos, si lo escrito tiene esa forma.
fn partes(consulta: &str) -> Option<(f64, String, String)> {
    let palabras: Vec<&str> = consulta.split_whitespace().collect();
    if palabras.len() < 3 {
        return None;
    }

    // Desde el final, igual que en las unidades: «100 eur en usd» tiene un
    // enlace solo, pero «100 a a usd» —«a» es una moneda válida de tres letras
    // en otras tablas— rompería buscando desde adelante.
    let corte = palabras
        .iter()
        .rposition(|palabra| ENLACES.contains(&palabra.to_lowercase().as_str()))?;

    let izquierda = &palabras[..corte];
    let derecha = &palabras[corte + 1..];

    if izquierda.len() < 2 || derecha.len() != 1 {
        return None;
    }

    let origen = codigo(izquierda[izquierda.len() - 1])?;
    let destino = codigo(derecha[0])?;

    // Lo de adelante de la moneda tiene que ser un número y nada más, como en
    // las unidades: en «cien usd a eur» no hay número, y en «abrir 100 usd a
    // eur» sobra una palabra.
    let cantidad = izquierda[..izquierda.len() - 1].join("");
    let cantidad: f64 = cantidad.replace(',', ".").parse().ok()?;

    Some((cantidad, origen, destino))
}

/// El importe como se escribe la plata.
///
/// No con el formateador de la calculadora. Éste se estrenó mostrando
/// `150939.017901999992 ARS` para «100 usd a pesos»: la cotización trae ocho
/// decimales y multiplicarla los arrastra todos, y con esa precisión el número
/// no se puede ni leer ni copiar a ningún lado donde sirva.
///
/// Dos decimales, que es como se escribe un importe, sin los ceros de más:
/// «87 EUR» y no «87.00 EUR». Por debajo de un centavo se pasa a cifras
/// significativas, porque ahí dos decimales serían siempre `0.00` — es el caso
/// de las criptomonedas, donde un dólar son diez millonésimas de bitcoin.
pub fn formatear_dinero(valor: f64) -> String {
    if !valor.is_finite() {
        return expresion::formatear(valor);
    }

    if valor != 0.0 && valor.abs() < 0.01 {
        // Ocho cifras significativas y afuera los ceros del final.
        let texto = format!(
            "{valor:.*}",
            8usize.saturating_add(valor.abs().log10().abs() as usize)
        );
        return texto
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_string();
    }

    let texto = format!("{valor:.2}");
    texto
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
}

/// La conversión como una fila de la lista.
pub fn resolver(consulta: &str, tabla: &Cotizaciones) -> Option<Resultado> {
    let conversion = convertir(consulta.trim(), tabla)?;
    let numero = formatear_dinero(conversion.valor);

    Some(Resultado {
        // Lo que se copia: el número solo, sin el código, que es lo que se pega
        // en un campo. Igual que en las cuentas.
        id: numero.clone(),
        accion: None,
        titulo: format!("{numero} {}", conversion.destino),
        // La fecha va **siempre**, no sólo cuando está vieja: quien no la ve la
        // toma por la de hoy, y desde acá no se sabe qué es viejo para quien
        // pregunta. La interfaz la traduce.
        subtitulo: Some("lanzador.cotizacionDel".to_string()),
        subtitulo_dato: Some(conversion.fecha),
        icono: Some(ICONO.to_string()),
        puntaje: PUNTAJE,
        origen: Origen::Calculo,
    })
}

/// Dónde vive lo guardado.
pub fn ruta_por_defecto() -> Option<PathBuf> {
    Some(crate::rutas::base(dirs::data_dir())?.join(ARCHIVO))
}

/// Lo que haya en el disco, o nada.
///
/// Que no haya es normal: nadie convirtió una moneda todavía. Que esté roto
/// también se trata como que no hay — se vuelve a bajar y se pisa.
pub fn del_disco(ruta: &std::path::Path) -> Option<Cotizaciones> {
    let contenido = std::fs::read_to_string(ruta).ok()?;
    serde_json::from_str(&contenido).ok()
}

/// Guarda lo traído, creando el directorio si hace falta.
pub fn al_disco(ruta: &std::path::Path, tabla: &Cotizaciones) -> Result<(), String> {
    if let Some(padre) = ruta.parent() {
        std::fs::create_dir_all(padre).map_err(|e| e.to_string())?;
    }
    let texto = serde_json::to_string(tabla).map_err(|e| e.to_string())?;
    std::fs::write(ruta, texto).map_err(|e| e.to_string())
}

/// Lo que contesta la fuente, que no es la forma en que se guarda.
#[derive(Deserialize)]
struct Respuesta {
    date: String,
    #[serde(flatten)]
    resto: HashMap<String, serde_json::Value>,
}

/// Interpreta lo que contestó la fuente.
///
/// Aparte del pedido para poder probarlo sin red, que es donde está todo lo que
/// se puede equivocar: la fuente devuelve `{"date": …, "usd": {…}}` y el nombre
/// de esa segunda clave es la moneda base, así que hay que sacarla por
/// descarte.
pub fn interpretar(cuerpo: &str) -> Option<Cotizaciones> {
    let respuesta: Respuesta = serde_json::from_str(cuerpo).ok()?;

    let tasas = respuesta
        .resto
        .values()
        .find_map(|valor| valor.as_object())?;

    let por_dolar: HashMap<String, f64> = tasas
        .iter()
        .filter_map(|(moneda, valor)| Some((moneda.to_lowercase(), valor.as_f64()?)))
        .collect();

    // Una tabla vacía no sirve y pisaría una buena: si la fuente contesta algo
    // que parsea pero no trae tasas, es como si no hubiera contestado.
    (!por_dolar.is_empty()).then_some(Cotizaciones {
        fecha: respuesta.date,
        por_dolar,
        // Lo pone `Monedas::guardar`, que es quien sabe cuándo se trajo.
        // Interpretar no mira el reloj: así se puede probar sin él.
        traido_en: 0,
    })
}

/// Trae las cotizaciones de la red.
///
/// Prueba las dos direcciones en orden: la documentación de la fuente pide
/// implementar el respaldo, y sin él una caída del CDN deja al proveedor
/// entero sin contestar.
pub fn de_la_red() -> Option<Cotizaciones> {
    let cliente = reqwest::blocking::Client::builder()
        .timeout(ESPERA)
        .user_agent(concat!("vasak-prism/", env!("CARGO_PKG_VERSION")))
        .build()
        .ok()?;

    for fuente in FUENTES {
        let Ok(respuesta) = cliente.get(*fuente).send() else {
            continue;
        };
        if !respuesta.status().is_success() {
            continue;
        }
        let Ok(cuerpo) = respuesta.text() else {
            continue;
        };
        if let Some(tabla) = interpretar(&cuerpo) {
            return Some(tabla);
        }
    }

    None
}

/// Cada cuánto se vuelve a pedir después de traerlas bien.
///
/// Veinte horas y no veinticuatro: la fuente publica una vez por día y a una
/// hora que no conocemos, así que con veinticuatro clavadas la actualización se
/// correría un poco cada día hasta caer siempre justo antes de la publicación.
const VIGENCIA: std::time::Duration = std::time::Duration::from_secs(20 * 60 * 60);

/// Cada cuánto se reintenta después de un intento fallido.
///
/// Sin esto, con la red caída cada consulta de moneda sería un pedido nuevo.
const ESPERA_TRAS_FALLAR: std::time::Duration = std::time::Duration::from_secs(60 * 60);

/// Las cotizaciones que Prism tiene a mano, y cuándo conviene volver a pedirlas.
pub struct Monedas {
    ruta: Option<PathBuf>,
    tabla: Option<Cotizaciones>,
    /// El último intento de esta sesión, haya salido bien o no. No se guarda en
    /// disco: lo que importa que sobreviva es el éxito, y eso ya va en la tabla.
    ultimo_intento: Option<std::time::SystemTime>,
}

impl Monedas {
    /// Lee lo que haya en el disco. **No toca la red.**
    pub fn nuevas(ruta: Option<PathBuf>) -> Self {
        let tabla = ruta.as_deref().and_then(del_disco);
        Self {
            ruta,
            tabla,
            ultimo_intento: None,
        }
    }

    pub fn tabla(&self) -> Option<&Cotizaciones> {
        self.tabla.as_ref()
    }

    /// Si conviene ir a buscarlas ahora.
    ///
    /// Aparte y con el tiempo por argumento porque es la decisión que se puede
    /// equivocar y no se ve: pedir de más es un lanzador que golpea una API
    /// ajena, y pedir de menos es una cotización vieja sin motivo. Con el reloj
    /// adentro habría que esperar veinte horas para probarla.
    pub fn conviene_intentar(&self, ahora: std::time::SystemTime) -> bool {
        // Un intento reciente manda sobre todo lo demás: con la red caída, sin
        // esto cada tecla que forme una conversión sería un pedido.
        if let Some(intento) = self.ultimo_intento {
            if ahora
                .duration_since(intento)
                .is_ok_and(|desde| desde < ESPERA_TRAS_FALLAR)
            {
                return false;
            }
        }

        let Some(tabla) = &self.tabla else {
            return true;
        };

        let traido = std::time::UNIX_EPOCH + std::time::Duration::from_secs(tabla.traido_en);
        match ahora.duration_since(traido) {
            Ok(desde) => desde >= VIGENCIA,
            // Traído «en el futuro»: el reloj se corrigió hacia atrás. Se pide
            // de nuevo, que es barato, en lugar de no volver a pedir nunca.
            Err(_) => true,
        }
    }

    /// Anota que se intentó, saliera como saliera.
    pub fn anotar_intento(&mut self, ahora: std::time::SystemTime) {
        self.ultimo_intento = Some(ahora);
    }

    /// Guarda lo traído, en memoria y en disco.
    ///
    /// Que no se pueda escribir no es motivo para no usarlas: se pierden al
    /// cerrar la sesión, nada más.
    pub fn guardar(&mut self, mut tabla: Cotizaciones, ahora: std::time::SystemTime) {
        tabla.traido_en = ahora
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(ruta) = &self.ruta {
            let _ = al_disco(ruta, &tabla);
        }
        self.tabla = Some(tabla);
    }
}

/// Trae las cotizaciones en otro hilo y las deja donde el lanzador las busca.
///
/// En otro hilo y sin devolver nada: quien la llama está atendiendo una
/// consulta y no puede esperar quince segundos. Lo que se trae aparece en la
/// próxima búsqueda.
///
/// Eso significa que la **primera** conversión de una sesión sin nada guardado
/// no muestra fila: la tabla llega un momento después y la siguiente tecla ya
/// la usa. Se podría avisar a la ventana para que rehaga la consulta sola, y no
/// se hace porque el caso dura lo que tarda escribir una letra y sólo pasa la
/// primera vez de la primera sesión. Con algo guardado —el caso normal— siempre
/// se contesta al instante, con lo que haya y su fecha.
pub fn refrescar_en_otro_hilo(monedas: std::sync::Arc<std::sync::Mutex<Monedas>>) {
    std::thread::spawn(move || {
        // El pedido se hace **fuera** del candado: son quince segundos de
        // espera como mucho, y con el candado tomado cada búsqueda de ese rato
        // se quedaría esperando a la red, que es exactamente lo que este diseño
        // evita.
        let Some(tabla) = de_la_red() else {
            return;
        };

        let mut guardadas = monedas.lock().unwrap_or_else(|e| e.into_inner());
        guardadas.guardar(tabla, std::time::SystemTime::now());
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tabla() -> Cotizaciones {
        Cotizaciones {
            fecha: "2026-09-21".to_string(),
            por_dolar: HashMap::from([
                ("usd".to_string(), 1.0),
                ("eur".to_string(), 0.87),
                ("ars".to_string(), 1514.66),
                ("brl".to_string(), 5.14),
            ]),
            traido_en: 0,
        }
    }

    #[test]
    fn una_conversion_por_codigo() {
        let c = convertir("100 usd a eur", &tabla()).expect("es una conversión");
        assert_eq!(c.destino, "EUR");
        assert!((c.valor - 87.0).abs() < 0.001, "{}", c.valor);
        assert_eq!(c.fecha, "2026-09-21");
    }

    #[test]
    fn y_por_nombre_en_castellano() {
        // «pesos» es ARS porque este sistema se usa acá. Está escrito en la
        // tabla, no deducido.
        let c = convertir("10 dolares a pesos", &tabla()).expect("es una conversión");
        assert_eq!(c.destino, "ARS");
        assert!((c.valor - 15146.6).abs() < 0.1, "{}", c.valor);
    }

    #[test]
    fn se_convierte_entre_dos_que_no_son_el_dolar() {
        // Todo pasa por el dólar, así que éste es el camino que de verdad se
        // puede equivocar: si se dividiera al revés daría un número plausible.
        let c = convertir("100 eur a brl", &tabla()).expect("es una conversión");
        // 100 EUR son 100/0.87 dólares, y eso por 5.14.
        assert!((c.valor - 590.8).abs() < 1.0, "{}", c.valor);
    }

    #[test]
    fn los_enlaces_que_se_escriben() {
        for consulta in [
            "100 usd a eur",
            "100 usd to eur",
            "100 usd en eur",
            "100 usd = eur",
        ] {
            assert!(
                convertir(consulta, &tabla()).is_some(),
                "«{consulta}» tendría que convertir"
            );
        }
    }

    #[test]
    fn una_moneda_que_la_tabla_no_tiene_no_es_una_conversion() {
        // Y no un error: la consulta tiene que poder seguir su camino y que la
        // busque otro proveedor.
        assert_eq!(convertir("100 usd a xyz", &tabla()), None);
    }

    #[test]
    fn lo_que_no_es_una_conversion_no_lo_parece() {
        for consulta in [
            "100 usd",             // falta el destino
            "usd a eur",           // falta la cantidad
            "cien usd a eur",      // la cantidad no es un número
            "abrir 100 usd a eur", // sobra una palabra adelante
            "100 usd a eur eur",   // sobra una atrás
            "firefox",             // ni cerca
            "2+2",                 // es una cuenta
        ] {
            assert_eq!(convertir(consulta, &tabla()), None, "«{consulta}»");
        }
    }

    #[test]
    fn la_coma_decimal_tambien() {
        // Acá se escribe con coma.
        let c = convertir("1,5 usd a eur", &tabla()).expect("es una conversión");
        assert!((c.valor - 1.305).abs() < 0.001, "{}", c.valor);
    }

    #[test]
    fn una_tasa_en_cero_no_da_infinito() {
        // Dividir por una tasa en cero daría infinito, y eso se mostraría como
        // una conversión perfectamente normal.
        let rota = Cotizaciones {
            fecha: "2026-09-21".to_string(),
            por_dolar: HashMap::from([("aaa".to_string(), 0.0), ("usd".to_string(), 1.0)]),
            traido_en: 0,
        };
        assert_eq!(convertir("100 aaa a usd", &rota), None);
    }

    #[test]
    fn la_fila_copia_el_numero_y_muestra_la_fecha() {
        let fila = resolver("100 usd a eur", &tabla()).expect("es una conversión");
        assert_eq!(fila.titulo, "87 EUR");
        assert_eq!(fila.id, "87", "lo que se copia es el número solo");
        assert_eq!(
            fila.subtitulo_dato.as_deref(),
            Some("2026-09-21"),
            "la fecha va siempre, no sólo cuando está vieja"
        );
        assert_eq!(fila.origen, Origen::Calculo);
    }

    #[test]
    fn se_interpreta_lo_que_contesta_la_fuente() {
        // La segunda clave del objeto es la moneda base y hay que sacarla por
        // descarte: no viene nombrada.
        let cuerpo = r#"{"date":"2026-09-20","usd":{"eur":0.87,"ars":1514.66,"USD":1}}"#;
        let tabla = interpretar(cuerpo).expect("se entiende");
        assert_eq!(tabla.fecha, "2026-09-20");
        assert_eq!(tabla.por_dolar.get("eur"), Some(&0.87));
        // Las claves se normalizan a minúsculas: la fuente mezcla.
        assert_eq!(tabla.por_dolar.get("usd"), Some(&1.0));
    }

    #[test]
    fn una_respuesta_sin_tasas_no_pisa_lo_que_habia() {
        // Si la fuente contesta algo que parsea y no trae tasas, es como si no
        // hubiera contestado: guardarla borraría la cotización vieja, que es
        // justo lo que sirve cuando no hay red.
        assert_eq!(interpretar(r#"{"date":"2026-09-20","usd":{}}"#), None);
        assert_eq!(interpretar(r#"{"date":"2026-09-20"}"#), None);
        assert_eq!(interpretar("no es json"), None);
    }

    #[test]
    fn lo_guardado_vuelve_igual() {
        let ruta = std::env::temp_dir().join(format!(
            "prism-cotizaciones-{}-{:?}.json",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_file(&ruta);

        assert_eq!(del_disco(&ruta), None, "todavía no hay nada");

        al_disco(&ruta, &tabla()).expect("se guarda");
        assert_eq!(del_disco(&ruta), Some(tabla()));

        std::fs::write(&ruta, "roto").unwrap();
        assert_eq!(del_disco(&ruta), None, "roto se trata como que no hay");

        let _ = std::fs::remove_file(&ruta);
    }

    fn en(segundos: u64) -> std::time::SystemTime {
        std::time::UNIX_EPOCH + std::time::Duration::from_secs(segundos)
    }

    #[test]
    fn sin_nada_guardado_conviene_pedir() {
        let m = Monedas::nuevas(None);
        assert!(m.conviene_intentar(en(1_000_000)));
    }

    #[test]
    fn con_algo_reciente_no_se_vuelve_a_pedir() {
        let mut m = Monedas::nuevas(None);
        m.guardar(tabla(), en(1_000_000));
        // Una hora después sigue valiendo.
        assert!(!m.conviene_intentar(en(1_000_000 + 3600)));
    }

    #[test]
    fn pasada_la_vigencia_se_vuelve_a_pedir() {
        let mut m = Monedas::nuevas(None);
        m.guardar(tabla(), en(1_000_000));
        assert!(m.conviene_intentar(en(1_000_000 + 21 * 3600)));
    }

    #[test]
    fn un_intento_fallido_no_se_repite_en_cada_tecla() {
        // Es el caso que importa: con la red caída no hay tabla, así que sin el
        // freno cada consulta que forme una conversión sería un pedido nuevo.
        let mut m = Monedas::nuevas(None);
        m.anotar_intento(en(1_000_000));
        assert!(!m.conviene_intentar(en(1_000_000 + 60)));
        assert!(!m.conviene_intentar(en(1_000_000 + 3500)));
        assert!(m.conviene_intentar(en(1_000_000 + 3700)));
    }

    #[test]
    fn una_fecha_del_futuro_no_congela_la_actualizacion() {
        // El reloj corregido hacia atrás dejaría lo guardado como si acabara de
        // traerse, y no se volvería a pedir nunca.
        let mut m = Monedas::nuevas(None);
        m.guardar(tabla(), en(2_000_000));
        assert!(m.conviene_intentar(en(1_000_000)));
    }

    #[test]
    fn se_reconoce_la_forma_sin_tener_tabla() {
        // Es lo que permite ir a buscar las cotizaciones la primera vez: hay que
        // saber que alguien pidió una conversión antes de tener con qué
        // resolverla.
        assert!(parece_conversion("100 usd a eur"));
        assert!(parece_conversion("10 dolares a pesos"));
        assert!(!parece_conversion("firefox"));
        assert!(!parece_conversion("2+2"));
        assert!(!parece_conversion("3 pulgadas a cm"));
    }

    #[test]
    fn el_importe_se_escribe_como_plata() {
        // Esto se estrenó mostrando `150939.017901999992 ARS`: la cotización
        // trae ocho decimales y multiplicarla los arrastra todos.
        assert_eq!(formatear_dinero(150_939.017_902), "150939.02");
        assert_eq!(formatear_dinero(295.205_184_081), "295.21");
        // Sin los ceros de más: «87» y no «87.00».
        assert_eq!(formatear_dinero(87.0), "87");
        assert_eq!(formatear_dinero(87.5), "87.5");
        assert_eq!(formatear_dinero(0.0), "0");
    }

    #[test]
    fn un_importe_menor_a_un_centavo_no_se_redondea_a_cero() {
        // Un dólar son diez millonésimas de bitcoin: con dos decimales todas
        // las conversiones a cripto darían `0`.
        let btc = formatear_dinero(1.230918e-5);
        assert_ne!(btc, "0", "quedó en cero: {btc}");
        assert!(btc.starts_with("0.0000123"), "{btc}");
    }
}
