//! El índice, de punta a punta, contra las aplicaciones de esta máquina.
//!
//! Las pruebas de unidad usan listas escritas a mano. Ésta comprueba lo que no
//! se puede escribir a mano: que lo que sale del disco entre y salga igual de la
//! caché, y que la caché recién escrita se dé por válida. Una caché que se
//! guarda y al leerla no vale es peor que no tenerla — el proceso reindexaría
//! en cada arranque sin que nada lo diga.

use vasak_prism_lib::catalogo::{self, cache, escaneo};

fn ruta_temporal(nombre: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!("prism-{}-{nombre}.db", std::process::id()))
}

#[test]
fn lo_que_sale_del_disco_entra_y_sale_igual_de_la_cache() {
    let escritorios = catalogo::escritorios_de("Vasak:wlroots");
    // Con la marca, que es la fecha del archivo más nuevo que el escaneo miró
    // —descartadas incluidas—. Sacarla de las aplicaciones guardadas es lo que
    // hacía que esta prueba fallara en cualquier máquina donde el `.desktop`
    // más nuevo del disco fuera uno que el escaneo descarta.
    let (aplicaciones, ultimo_visto) = escaneo::escanear_con_marca("es_AR", &escritorios);

    if aplicaciones.is_empty() {
        // Una máquina sin aplicaciones instaladas. No hay nada que comprobar y
        // eso no es un error.
        return;
    }

    let ruta = ruta_temporal("ida-y-vuelta");
    let _ = std::fs::remove_file(&ruta);

    let mut almacen = cache::Cache::abrir(&ruta).expect("abrir la caché");
    almacen
        .guardar(&aplicaciones, ultimo_visto)
        .expect("guardar");

    let releidas = cache::Cache::abrir(&ruta).expect("reabrir").leer();
    assert_eq!(releidas, aplicaciones);

    // Y recién escrita tiene que darse por válida: si no, el proceso reindexa
    // en cada arranque y la caché no sirve para nada.
    assert!(cache::esta_al_dia(
        &releidas,
        &escaneo::archivos(),
        ultimo_visto
    ));

    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn el_catalogo_arranca_de_la_cache_y_busca() {
    let ruta = ruta_temporal("catalogo");
    let _ = std::fs::remove_file(&ruta);

    let catalogo = catalogo::Catalogo::nuevo(
        "es_AR".to_string(),
        catalogo::escritorios_de("Vasak:wlroots"),
        Some(ruta.clone()),
    );

    // Sin caché todavía: no falla, queda vacío.
    assert_eq!(catalogo.cargar_de_cache(), 0);
    assert!(catalogo.buscar("a", 10).is_empty());

    catalogo.reindexar();
    let indexadas = catalogo.aplicaciones().len();
    if indexadas == 0 {
        return;
    }

    // Y ahora un catálogo nuevo, del mismo archivo, tiene que arrancar lleno.
    let otro = catalogo::Catalogo::nuevo(
        "es_AR".to_string(),
        catalogo::escritorios_de("Vasak:wlroots"),
        Some(ruta.clone()),
    );
    assert_eq!(otro.cargar_de_cache(), indexadas);
    assert!(otro.esta_al_dia());

    // Reindexar sobre algo que no cambió no es novedad para nadie.
    assert!(!otro.reindexar());

    let _ = std::fs::remove_file(&ruta);
}

#[test]
fn sin_caché_en_disco_tampoco_se_reindexa_en_cada_comprobación() {
    // El mismo agujero por el otro camino. Sin `ruta_cache` no hay dónde
    // guardar la marca, pero el escaneo igual la calcula: si se tira, la
    // comprobación siguiente lee cero, cualquier archivo descartado cuenta como
    // novedad, y se vuelve a escanear el disco entero cada vez que alguien
    // pregunta.
    //
    // Se apoya en el disco de esta máquina, igual que las de arriba: sin
    // aplicaciones instaladas no hay nada que comprobar y eso no es un error.
    let catalogo = catalogo::Catalogo::nuevo(
        "es_AR".to_string(),
        catalogo::escritorios_de("Vasak:wlroots"),
        None,
    );

    catalogo.reindexar();

    if catalogo.aplicaciones().is_empty() {
        return;
    }

    assert!(
        catalogo.esta_al_dia(),
        "recién escaneado tiene que darse por válido aunque no haya caché"
    );
}
