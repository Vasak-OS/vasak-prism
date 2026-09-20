//! Los catálogos de idioma tienen que parsear y estar completos.
//!
//! Este test existe porque su ausencia costó una aplicación que no abría: un
//! valor sin comillas que contenía `: ` hacía que el parser lo leyera como un
//! mapeo anidado, rompía el archivo entero y el plugin de i18n paniqueaba al
//! arrancar. Ninguna compilación lo detecta.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Los idiomas que la plantilla trae. Agregá el tuyo acá al agregar el `.yml`.
const IDIOMAS: &[&str] = &["es", "en"];

fn ruta(idioma: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("locales")
        .join(format!("{idioma}.yml"))
}

fn cargar(idioma: &str) -> serde_yaml::Value {
    let texto = std::fs::read_to_string(ruta(idioma))
        .unwrap_or_else(|error| panic!("no se pudo leer {idioma}.yml: {error}"));
    let valor: serde_yaml::Value = serde_yaml::from_str(&texto)
        .unwrap_or_else(|error| panic!("{idioma}.yml no parsea: {error}"));

    // Tiene que ser un mapeo, no cualquier YAML válido: un catálogo que quede en
    // `null` parsea perfecto y haría pasar todo lo de abajo sin revisar nada.
    assert!(
        matches!(valor, serde_yaml::Value::Mapping(_)),
        "{idioma}.yml no es un mapeo en la raíz"
    );
    valor
}

/// Todas las claves como `grupo.clave`, con su texto.
fn textos(valor: &serde_yaml::Value) -> BTreeMap<String, String> {
    let mut salida = BTreeMap::new();
    recorrer(valor, "", &mut salida);
    salida
}

/// Aplana el catálogo a `grupo.subgrupo.clave`, a cualquier profundidad.
///
/// Recursivo y no de dos niveles fijos, que es como estaba. Un grupo anidado más
/// abajo quedaba **sin comprobar** y además aparecía como un texto vacío, porque un
/// mapeo no es una cadena: el test fallaba justo donde no miraba. Lo encontró
/// vasak-monitor al agrupar sus tareas de limpieza en un tercer nivel.
fn recorrer(valor: &serde_yaml::Value, prefijo: &str, salida: &mut BTreeMap<String, String>) {
    match valor {
        serde_yaml::Value::Mapping(mapa) => {
            for (clave, hijo) in mapa {
                let nombre = clave.as_str().unwrap_or_default();
                let completa = if prefijo.is_empty() {
                    nombre.to_string()
                } else {
                    format!("{prefijo}.{nombre}")
                };
                recorrer(hijo, &completa, salida);
            }
        }
        serde_yaml::Value::String(texto) => {
            salida.insert(prefijo.to_string(), texto.clone());
        }
        // Un número o un booleano en un catálogo de textos es un error de tipeo,
        // pero no es un texto vacío: se ignora en lugar de fallar por él.
        _ => {}
    }
}

/// Los marcadores `{n}` **cerrados** de un texto, conservando repeticiones.
fn marcadores(texto: &str) -> Vec<String> {
    let bytes = texto.as_bytes();
    let mut salida = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'{' {
            i += 1;
            continue;
        }
        let mut j = i + 1;
        while j < bytes.len() && bytes[j].is_ascii_digit() {
            j += 1;
        }
        // Al menos un dígito y un `}` justo después: un `{0` sin cerrar no
        // cuenta, que es el error que un parseo perezoso deja pasar.
        if j > i + 1 && j < bytes.len() && bytes[j] == b'}' {
            salida.push(texto[i..=j].to_string());
            i = j + 1;
        } else {
            i += 1;
        }
    }
    salida.sort();
    salida
}

#[test]
fn los_catalogos_parsean() {
    for idioma in IDIOMAS {
        cargar(idioma);
    }
}

#[test]
fn todos_los_idiomas_tienen_las_mismas_claves() {
    let referencia: BTreeSet<String> = textos(&cargar(IDIOMAS[0])).into_keys().collect();

    for idioma in &IDIOMAS[1..] {
        let claves: BTreeSet<String> = textos(&cargar(idioma)).into_keys().collect();
        let faltan: Vec<_> = referencia.difference(&claves).collect();
        let sobran: Vec<_> = claves.difference(&referencia).collect();

        // Una clave que falta se muestra cruda en la interfaz.
        assert!(faltan.is_empty(), "{idioma} no tiene: {faltan:?}");
        assert!(sobran.is_empty(), "{idioma} tiene de más: {sobran:?}");
    }
}

#[test]
fn ningun_texto_queda_vacio() {
    // Un hueco en la interfaz es peor que la clave cruda: la clave al menos se
    // nota y se puede buscar.
    for idioma in IDIOMAS {
        for (clave, texto) in textos(&cargar(idioma)) {
            assert!(!texto.trim().is_empty(), "{idioma}: {clave} está vacía");
        }
    }
}

#[test]
fn los_marcadores_coinciden_entre_idiomas() {
    // Si un idioma dice `{0}` y el otro no, una de las dos traducciones muestra
    // el marcador crudo o pierde el dato. Con `{0}` reemplazado a mano, nada más
    // lo detecta.
    let referencia = textos(&cargar(IDIOMAS[0]));

    for idioma in &IDIOMAS[1..] {
        let otros = textos(&cargar(idioma));
        for (clave, texto) in &referencia {
            let Some(otro) = otros.get(clave) else { continue };
            assert_eq!(
                marcadores(texto),
                marcadores(otro),
                "los marcadores de {clave} no coinciden entre {} y {idioma}",
                IDIOMAS[0]
            );
        }
    }
}
