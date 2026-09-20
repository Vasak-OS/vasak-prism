//! Lo que se usa seguido aparece primero.
//!
//! Es la diferencia entre escribir tres letras y escribir una. Sin esto, la
//! décima vez que abrís el navegador el orden es el mismo que la primera: el
//! lanzador no aprende nunca, y la aplicación que usás todos los días compite de
//! igual a igual con una que instalaste y no volviste a abrir.
//!
//! Va en `$XDG_DATA_HOME` y **no** en la caché. Esto no es contenido derivado:
//! si se borra, se pierde. La caché del índice se rehace sola leyendo el disco;
//! esto no se rehace con nada.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{Connection, OptionalExtension};

/// Cada cuántos días vale la mitad.
///
/// Un mes: lo que usabas todos los días en marzo y no volviste a abrir no puede
/// seguir ganándole en junio a lo que usás ahora.
const MEDIA_VIDA_DIAS: f64 = 30.0;

/// Cuánto puede levantar la frecuencia el puntaje de texto, como fracción.
///
/// Medio punto sobre uno. Con eso, lo más usado sube mucho pero una coincidencia
/// exacta de nombre —que vale 100— le sigue ganando a cualquier coincidencia
/// parcial por muy usada que esté: 65 × 1,5 son 97,5.
const EMPUJE: f64 = 0.5;

const SEGUNDOS_POR_DIA: f64 = 86_400.0;

/// Dónde vive: `$XDG_DATA_HOME/vasak-prism/uso.db`.
pub fn ruta_por_defecto() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_DATA_HOME") {
        Some(valor) if !valor.is_empty() => PathBuf::from(valor),
        _ => PathBuf::from(std::env::var_os("HOME")?).join(".local/share"),
    };

    Some(base.join("vasak-prism").join("uso.db"))
}

/// La clave de una fila: la aplicación, o la aplicación y su acción.
///
/// Separadas porque son dos cosas distintas: quien abre siempre la ventana
/// privada de Firefox no necesariamente abre Firefox.
pub fn clave(id: &str, accion: Option<&str>) -> String {
    match accion {
        Some(accion) => format!("{id}#{accion}"),
        None => id.to_string(),
    }
}

pub struct Uso {
    conexion: Connection,
}

impl Uso {
    pub fn abrir(ruta: &Path) -> Result<Self, String> {
        if let Some(directorio) = ruta.parent() {
            std::fs::create_dir_all(directorio).map_err(|e| e.to_string())?;
        }

        let conexion = Connection::open(ruta).map_err(|e| e.to_string())?;
        let uso = Self { conexion };
        uso.preparar()?;
        Ok(uso)
    }

    #[cfg(test)]
    pub fn en_memoria() -> Result<Self, String> {
        let conexion = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let uso = Self { conexion };
        uso.preparar()?;
        Ok(uso)
    }

    fn preparar(&self) -> Result<(), String> {
        self.conexion
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS uso (
                     clave TEXT PRIMARY KEY,
                     veces INTEGER NOT NULL DEFAULT 0,
                     ultimo INTEGER NOT NULL DEFAULT 0
                 );",
            )
            .map_err(|e| e.to_string())
    }

    /// Anota que se lanzó esto, ahora.
    pub fn anotar(&self, clave: &str) -> Result<(), String> {
        self.anotar_en(clave, ahora())
    }

    /// Igual, con el momento dado. Es lo que permite probar el decaimiento sin
    /// esperar un mes.
    pub fn anotar_en(&self, clave: &str, cuando: i64) -> Result<(), String> {
        self.conexion
            .execute(
                "INSERT INTO uso (clave, veces, ultimo) VALUES (?1, 1, ?2)
                 ON CONFLICT(clave) DO UPDATE SET veces = veces + 1, ultimo = ?2",
                rusqlite::params![clave, cuando],
            )
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    pub fn veces(&self, clave: &str) -> i64 {
        self.conexion
            .query_row("SELECT veces FROM uso WHERE clave = ?1", [clave], |fila| {
                fila.get(0)
            })
            .optional()
            .ok()
            .flatten()
            .unwrap_or(0)
    }

    /// El peso de cada clave, ya normalizado entre 0 y 1.
    pub fn pesos(&self) -> HashMap<String, f64> {
        self.pesos_en(ahora())
    }

    pub fn pesos_en(&self, cuando: i64) -> HashMap<String, f64> {
        let Ok(mut consulta) = self
            .conexion
            .prepare("SELECT clave, veces, ultimo FROM uso")
        else {
            return HashMap::new();
        };

        let filas = consulta.query_map([], |fila| {
            Ok((
                fila.get::<_, String>(0)?,
                fila.get::<_, i64>(1)?,
                fila.get::<_, i64>(2)?,
            ))
        });

        let Ok(filas) = filas else {
            return HashMap::new();
        };

        let crudos: Vec<(String, f64)> = filas
            .flatten()
            .map(|(clave, veces, ultimo)| (clave, peso(veces, ultimo, cuando)))
            .collect();

        // Normalizado contra el más usado y no contra un número fijo: lo que
        // importa es el orden entre lo que hay, y cuánto usa la máquina cada
        // persona no se puede saber de antemano.
        let mayor = crudos.iter().map(|(_, peso)| *peso).fold(0.0_f64, f64::max);

        if mayor <= 0.0 {
            return HashMap::new();
        }

        crudos
            .into_iter()
            .map(|(clave, crudo)| (clave, crudo / mayor))
            .collect()
    }
}

fn ahora() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|desde| desde.as_secs() as i64)
        .unwrap_or(0)
}

/// Cuánto vale una fila: las veces que se usó, apagándose con el tiempo.
///
/// Logarítmico en la cantidad para que la número cien no valga cien veces más
/// que la primera —lo que se usa a diario se dispararía y taparía todo lo demás—
/// y con media vida en el tiempo, que es lo que hace que el lanzador se olvide
/// de lo que dejaste de usar.
fn peso(veces: i64, ultimo: i64, cuando: i64) -> f64 {
    if veces <= 0 {
        return 0.0;
    }

    let dias = ((cuando - ultimo).max(0) as f64) / SEGUNDOS_POR_DIA;
    let olvido = 0.5_f64.powf(dias / MEDIA_VIDA_DIAS);

    ((veces + 1) as f64).ln() * olvido
}

/// El puntaje de texto, levantado por lo que se usa.
///
/// Multiplica en vez de sumar a propósito: lo que no coincide con nada sigue sin
/// coincidir por mucho que se use, y una aplicación muy usada no se cuela en una
/// búsqueda que no la nombra.
pub fn con_uso(puntaje: f64, peso: f64) -> f64 {
    puntaje * (1.0 + EMPUJE * peso.clamp(0.0, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIA: i64 = 86_400;

    #[test]
    fn lo_que_no_se_uso_nunca_no_pesa() {
        let uso = Uso::en_memoria().unwrap();
        assert!(uso.pesos().is_empty());
        assert_eq!(uso.veces("firefox.desktop"), 0);
    }

    #[test]
    fn anotar_suma() {
        let uso = Uso::en_memoria().unwrap();
        uso.anotar("firefox.desktop").unwrap();
        uso.anotar("firefox.desktop").unwrap();

        assert_eq!(uso.veces("firefox.desktop"), 2);
    }

    #[test]
    fn lo_de_ayer_le_gana_a_lo_del_mes_pasado() {
        // El punto del decaimiento: lo que usabas todos los días en marzo no
        // puede seguir ganándole en junio a lo que usás ahora.
        let uso = Uso::en_memoria().unwrap();
        let hoy = 100 * DIA;

        for _ in 0..20 {
            uso.anotar_en("viejo.desktop", hoy - 60 * DIA).unwrap();
        }
        for _ in 0..3 {
            uso.anotar_en("nuevo.desktop", hoy - DIA).unwrap();
        }

        let pesos = uso.pesos_en(hoy);
        assert!(pesos["nuevo.desktop"] > pesos["viejo.desktop"]);
    }

    #[test]
    fn a_igual_antiguedad_gana_el_mas_usado() {
        let uso = Uso::en_memoria().unwrap();
        let hoy = 100 * DIA;

        for _ in 0..10 {
            uso.anotar_en("mucho.desktop", hoy - DIA).unwrap();
        }
        uso.anotar_en("poco.desktop", hoy - DIA).unwrap();

        let pesos = uso.pesos_en(hoy);
        assert!(pesos["mucho.desktop"] > pesos["poco.desktop"]);
    }

    #[test]
    fn el_mas_usado_vale_uno() {
        let uso = Uso::en_memoria().unwrap();
        let hoy = 100 * DIA;
        uso.anotar_en("uno.desktop", hoy).unwrap();
        uso.anotar_en("dos.desktop", hoy - 10 * DIA).unwrap();

        let pesos = uso.pesos_en(hoy);
        assert_eq!(pesos["uno.desktop"], 1.0);
        assert!(pesos["dos.desktop"] < 1.0);
    }

    #[test]
    fn la_centesima_vez_no_vale_cien_veces_mas_que_la_primera() {
        // Logarítmico: si no, lo que se usa a diario se dispara y tapa todo lo
        // demás para siempre.
        let hoy = 100 * DIA;
        assert!(peso(100, hoy, hoy) < 10.0 * peso(1, hoy, hoy));
    }

    #[test]
    fn la_accion_cuenta_aparte_de_su_aplicacion() {
        // Quien abre siempre la ventana privada no necesariamente abre Firefox.
        assert_eq!(clave("firefox.desktop", None), "firefox.desktop");
        assert_eq!(
            clave("firefox.desktop", Some("privada")),
            "firefox.desktop#privada"
        );

        let uso = Uso::en_memoria().unwrap();
        uso.anotar(&clave("firefox.desktop", Some("privada")))
            .unwrap();

        assert_eq!(uso.veces("firefox.desktop"), 0);
        assert_eq!(uso.veces("firefox.desktop#privada"), 1);
    }

    #[test]
    fn el_uso_levanta_el_puntaje_pero_no_inventa_coincidencias() {
        // Lo que no coincide con nada sigue sin coincidir por mucho que se use.
        assert_eq!(con_uso(0.0, 1.0), 0.0);
        assert!(con_uso(50.0, 1.0) > con_uso(50.0, 0.0));
    }

    #[test]
    fn una_coincidencia_exacta_le_gana_a_una_suelta_muy_usada() {
        // El piso de previsibilidad: algo que apenas contiene lo que escribiste
        // no puede pasarle adelante a algo que se llama exactamente así, por muy
        // usado que esté. Con un empuje de 0,5, para superar a un 100 hace falta
        // partir de 66,7, y «contiene» vale 65.
        const EXACTO: f64 = 100.0;
        const CONTIENE: f64 = 65.0;

        assert!(con_uso(EXACTO, 0.0) > con_uso(CONTIENE, 1.0));
    }

    #[test]
    fn pero_una_palabra_del_nombre_muy_usada_si_puede_ganar() {
        // Y está bien que pueda: escribís «code», tenés una aplicación llamada
        // «Code» que no abriste nunca y «Visual Studio Code» que usás todos los
        // días. La que querías es la segunda. Queda anotado porque es la
        // consecuencia menos obvia del empuje y no un descuido.
        const EXACTO: f64 = 100.0;
        const PALABRA: f64 = 80.0;

        assert!(con_uso(PALABRA, 1.0) > con_uso(EXACTO, 0.0));
    }

    #[test]
    fn el_empuje_no_se_desborda_con_un_peso_fuera_de_rango() {
        assert_eq!(con_uso(100.0, 5.0), con_uso(100.0, 1.0));
        assert_eq!(con_uso(100.0, -3.0), 100.0);
    }
}
