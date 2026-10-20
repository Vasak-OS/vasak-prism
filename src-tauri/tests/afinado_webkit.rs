//! Prueba de guardia del afinado del WebKit.
//!
//! El glue con GTK/WebKit no se puede ejercitar sin un display ni un WebKit
//! reales, así que vive en `commands/webkit.rs` —que la cobertura excluye— y no
//! lo cubre ninguna prueba de ejecución. Lo que sí se puede cuidar, y es donde
//! el cableado se rompe en silencio, es que las piezas sigan conectadas: la
//! función y sus palancas en el fuente, el comando registrado en `lib.rs` y la
//! llamada del frontend al montar. Esta prueba lee el fuente y lo comprueba; no
//! toca GTK.

use std::path::PathBuf;

fn leer(ruta_relativa: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(ruta_relativa);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()))
}

/// El fuente del afinado tiene la función pública y las palancas seguras.
#[test]
fn el_glue_tiene_la_funcion_y_las_palancas() {
    let fuente = leer("src/commands/webkit.rs");
    for aguja in [
        "pub fn tune_webview",
        "set_enable_developer_extras(false)",
        "set_enable_back_forward_navigation_gestures(false)",
        "CacheModel::DocumentViewer",
    ] {
        assert!(
            fuente.contains(aguja),
            "`commands/webkit.rs` no contiene «{aguja}»"
        );
    }
}

/// El comando está registrado en el `generate_handler!` de `lib.rs`.
#[test]
fn el_comando_esta_registrado() {
    let fuente = leer("src/lib.rs");
    assert!(
        fuente.contains("commands::webkit::tune_webview"),
        "`lib.rs` no registra `commands::webkit::tune_webview` en el handler"
    );
}

/// El frontend lo invoca al montar la ventana.
#[test]
fn el_frontend_lo_invoca() {
    let fuente = leer("../src/App.vue");
    assert!(
        fuente.contains("invoke('tune_webview')"),
        "`App.vue` no invoca `tune_webview` al montar"
    );
}
