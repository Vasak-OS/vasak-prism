//! El parser contra los `.desktop` de esta máquina.
//!
//! Las pruebas de unidad usan archivos escritos a mano, que son los casos que
//! uno se imagina. Los `.desktop` de verdad traen los que no: claves repetidas,
//! idiomas con modificador, acciones a medio declarar, valores con `=` adentro,
//! archivos de paquetes viejos. Si el parser se cae con alguno, se cae con el
//! catálogo entero y el lanzador abre vacío.
//!
//! No falla en una máquina sin aplicaciones instaladas —la CI puede ser una—:
//! ahí no hay nada que comprobar y eso no es un error.

use std::fs;
use std::path::PathBuf;

use vasak_prism_lib::catalogo::{entrada, exec};

fn directorios() -> Vec<PathBuf> {
    let mut rutas = vec![
        PathBuf::from("/usr/share/applications"),
        PathBuf::from("/usr/local/share/applications"),
    ];
    if let Ok(home) = std::env::var("HOME") {
        rutas.push(PathBuf::from(home).join(".local/share/applications"));
    }
    rutas.into_iter().filter(|ruta| ruta.is_dir()).collect()
}

fn archivos() -> Vec<PathBuf> {
    directorios()
        .iter()
        .filter_map(|directorio| fs::read_dir(directorio).ok())
        .flatten()
        .flatten()
        .map(|entrada| entrada.path())
        .filter(|ruta| ruta.extension().is_some_and(|ext| ext == "desktop"))
        .collect()
}

#[test]
fn el_catalogo_real_se_lee_entero() {
    let archivos = archivos();
    if archivos.is_empty() {
        return;
    }

    let escritorios = vec!["Vasak".to_string()];
    let mut leidas = 0;

    for ruta in &archivos {
        let Ok(contenido) = fs::read_to_string(ruta) else {
            continue;
        };
        let id = ruta.file_name().unwrap().to_string_lossy().to_string();

        if entrada::parsear(&contenido, &id, "es_AR", &escritorios).is_some() {
            leidas += 1;
        }
    }

    // El filtrado saca bastantes —`NoDisplay` solo es la mitad en muchos
    // sistemas—, pero si no quedara ninguna el lanzador abriría vacío y eso no
    // es un catálogo filtrado, es un parser roto.
    assert!(
        leidas > 0,
        "ninguno de los {} archivos del sistema dio una entrada",
        archivos.len()
    );
}

#[test]
fn todo_lo_que_se_lee_se_puede_lanzar() {
    let escritorios = vec!["Vasak".to_string()];

    for ruta in archivos() {
        let Ok(contenido) = fs::read_to_string(&ruta) else {
            continue;
        };
        let id = ruta.file_name().unwrap().to_string_lossy().to_string();

        let Some(leida) = entrada::parsear(&contenido, &id, "es_AR", &escritorios) else {
            continue;
        };

        // Una entrada que se muestra y no se puede desarmar es una fila que al
        // apretar Enter no hace nada, que es peor que no mostrarla.
        assert!(
            exec::desarmar(
                &leida.exec,
                &leida.nombre,
                leida.icono.as_deref(),
                &ruta.to_string_lossy()
            )
            .is_some(),
            "{id}: el Exec «{}» no se puede desarmar",
            leida.exec
        );

        assert!(!leida.nombre.trim().is_empty(), "{id}: nombre vacío");

        for accion in &leida.acciones {
            assert!(
                exec::desarmar(
                    &accion.exec,
                    &accion.nombre,
                    accion.icono.as_deref(),
                    &ruta.to_string_lossy()
                )
                .is_some(),
                "{id}: la acción «{}» no se puede desarmar",
                accion.id
            );
        }
    }
}
