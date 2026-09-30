//! Que la lista de dependencias del `.deb` sea la de esta aplicación.
//!
//! Venía copiada de la plantilla `vapp`, igual que en otras diez del taller:
//! `libsoup2.4-1` junto con `libsoup-3.0-0` —las dos generaciones, y el binario
//! sólo enlaza la 3— y `libpango-1.0-0` sin enlazarla. Le faltaba lo que el
//! binario sí enlaza y la plantilla no conoce: `gtk-layer-shell` para la
//! ventana centrada y encima de todo, SQLite para la caché del índice, D-Bus y
//! JavaScriptCore. Y tampoco declaraba lo que la receta de Arch trae para usar
//! en ejecución: `wl-copy`, `xdg-open` y `systemd-run`.
//!
//! Lo que se declara se audita con `readelf -d … | grep NEEDED`, nunca con
//! `ldd`. Estas pruebas no reemplazan esa auditoría: cuidan que no vuelva lo
//! que se sacó y que no falte lo que se sabe que se enlaza.

use std::path::PathBuf;

fn deb_depends() -> Vec<String> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tauri.conf.json");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("no se pudo leer {}: {e}", path.display()));
    let config: serde_json::Value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("{} no es JSON válido: {e}", path.display()));
    config["bundle"]["linux"]["deb"]["depends"]
        .as_array()
        .expect("bundle.linux.deb.depends tiene que existir")
        .iter()
        .map(|v| {
            v.as_str()
                .expect("cada dependencia es un texto")
                .to_string()
        })
        .collect()
}

#[test]
fn las_dependencias_del_deb_tienen_nombre_de_debian() {
    for name in deb_depends() {
        // Los nombres de paquete de Debian: minúsculas, dígitos y `+-.`.
        let valid = name.len() >= 2
            && name
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphanumeric())
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+-.".contains(c));
        assert!(valid, "«{name}» no es un nombre de paquete de Debian");
        assert!(
            !name.ends_with("-devel") && !name.starts_with("gstreamer1-"),
            "«{name}» es un nombre de Fedora, no de Debian"
        );
        assert!(
            !name.ends_with("-dev"),
            "«{name}» es de compilación: el paquete instalado no lo usa"
        );
    }
}

#[test]
fn las_dependencias_del_deb_no_se_repiten() {
    let mut seen = std::collections::BTreeSet::new();
    for name in deb_depends() {
        assert!(seen.insert(name.clone()), "«{name}» está dos veces");
    }
}

/// El binario enlaza libsoup 3 y nada más: la 2.4 viene de la plantilla.
#[test]
fn no_viajan_las_dos_generaciones_de_libsoup() {
    assert!(
        !deb_depends().iter().any(|n| n == "libsoup2.4-1"),
        "libsoup2.4-1 no la enlaza nadie: el binario usa libsoup-3.0"
    );
}

/// Lo que la plantilla traía y el binario no enlaza. Si vuelve a enlazarse,
/// se saca de acá y se agrega a la lista de abajo a la vez.
#[test]
fn no_esta_la_que_el_binario_no_enlaza() {
    assert!(
        !deb_depends().iter().any(|n| n == "libpango-1.0-0"),
        "libpango-1.0-0 está en el .deb y el binario no la enlaza"
    );
}

/// Lo que `readelf -d` muestra enlazado, con su paquete de Debian. Si una de
/// éstas deja de enlazarse, se saca de la lista y de acá a la vez.
#[test]
fn estan_las_bibliotecas_que_el_binario_enlaza() {
    let depends = deb_depends();
    for (soname, package) in [
        ("libcairo.so.2", "libcairo2"),
        ("libdbus-1.so.3", "libdbus-1-3"),
        ("libgdk_pixbuf-2.0.so.0", "libgdk-pixbuf-2.0-0"),
        ("libglib-2.0.so.0", "libglib2.0-0t64"),
        ("libgtk-3.so.0", "libgtk-3-0t64"),
        ("libgtk-layer-shell.so.0", "libgtk-layer-shell0"),
        (
            "libjavascriptcoregtk-4.1.so.0",
            "libjavascriptcoregtk-4.1-0",
        ),
        ("libsoup-3.0.so.0", "libsoup-3.0-0"),
        ("libsqlite3.so.0", "libsqlite3-0"),
        ("libwebkit2gtk-4.1.so.0", "libwebkit2gtk-4.1-0"),
    ] {
        assert!(
            depends.iter().any(|n| n == package),
            "el binario enlaza {soname} y el .deb no declara {package}"
        );
    }
}

/// Lo que no se enlaza pero la receta de Arch declara porque se usa en
/// ejecución: el daemon por D-Bus, `systemd-run` para lanzar cada programa en
/// su ámbito, `wl-copy` para copiar un resultado y `xdg-open` para abrirlo.
#[test]
fn estan_los_programas_que_se_usan_sin_enlazarlos() {
    let depends = deb_depends();
    for package in ["dbus", "systemd", "wl-clipboard", "xdg-utils"] {
        assert!(
            depends.iter().any(|n| n == package),
            "falta {package} en el .deb"
        );
    }
}
