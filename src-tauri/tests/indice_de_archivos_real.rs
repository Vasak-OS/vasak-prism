//! El contrato del índice, contra el índice de verdad de esta máquina.
//!
//! Las pruebas de unidad comprueban que el lanzador se entiende consigo mismo,
//! que es poco: el esquema lo escriben las dos y por eso coinciden. Lo que no
//! se puede escribir a mano es lo único que importa acá — que un índice escrito
//! por **`vasak-file-manager`**, compilado aparte y con su propia copia del
//! contrato, sea el mismo que este programa sabe abrir.
//!
//! # Qué pasa si no lo es
//!
//! `abrir_para_escribir` descarta el índice entero cuando el esquema guardado
//! no es el suyo, y el gestor hace lo mismo del otro lado. O sea que un
//! contrato roto no da un error: da **dos programas que se borran el índice el
//! uno al otro**, cada uno rehaciéndolo en cuanto el otro termina, recorriendo
//! el disco entero cada vez. Nadie ve un fallo; se ve un escritorio que muele
//! disco sin motivo.
//!
//! Por eso esta prueba mira que el índice ajeno **se conserve**.

use std::path::PathBuf;

use vasak_prism_lib::proveedores::archivos::{contrato, escaneo};

/// Copia un directorio, sin recursión: un índice de tantivy es plano.
fn copiar(desde: &std::path::Path, hasta: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(hasta)?;
    for entrada in std::fs::read_dir(desde)? {
        let entrada = entrada?;
        if entrada.file_type()?.is_file() {
            std::fs::copy(entrada.path(), hasta.join(entrada.file_name()))?;
        }
    }
    Ok(())
}

#[test]
fn un_indice_escrito_por_el_gestor_no_se_descarta() {
    let Some(base) = contrato::base_de_cache() else {
        return;
    };
    let real = contrato::directorio_del_indice(&base);
    if !real.is_dir() {
        // Nadie escaneó nunca en esta máquina, o es la CI. No hay nada que
        // comprobar y eso no es un error.
        return;
    }

    let Ok(indice_real) = tantivy::Index::open_in_dir(&real) else {
        return;
    };
    let Ok(lector) = indice_real.reader() else {
        return;
    };
    let cuantos_antes = lector.searcher().num_docs();
    if cuantos_antes == 0 {
        // Un índice vacío no distingue «se conservó» de «se rehizo».
        return;
    }
    drop(lector);
    drop(indice_real);

    // Sobre una copia: esta prueba no puede tocar el índice del escritorio.
    let copia = std::env::temp_dir().join(format!(
        "prism-indice-ajeno-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let _ = std::fs::remove_dir_all(&copia);
    copiar(&real, &copia).expect("copiar el índice");

    let abierto = escaneo::abrir_para_escribir(&copia).expect("tiene que abrirse");
    let cuantos_despues = abierto.reader().expect("lector").searcher().num_docs();

    assert_eq!(
        cuantos_despues, cuantos_antes,
        "el índice del gestor se descartó al abrirlo: los esquemas no coinciden"
    );

    let _ = std::fs::remove_dir_all(&copia);
}

#[test]
fn lo_que_escribe_el_lanzador_lo_vuelve_a_leer_el_proveedor() {
    // La otra mitad, y la que sí corre en cualquier máquina: escanear un árbol
    // de verdad y buscarlo con el mismo código con el que el gestor lo va a
    // leer. Sin esto, el escaneo podría escribir algo que nadie puede consultar.
    let base = std::env::temp_dir().join(format!("prism-ida-y-vuelta-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let disco = base.join("disco");
    std::fs::create_dir_all(&disco).expect("armar el árbol");
    std::fs::write(disco.join("memoria anual.odt"), b"x").expect("escribir");

    let cuantas = escaneo::escanear(
        &base,
        std::slice::from_ref(&disco),
        &[],
        &std::sync::atomic::AtomicBool::new(false),
        escaneo::ahora_ms(),
    )
    .expect("escanear");
    assert_eq!(cuantas, 1);

    let indice = vasak_prism_lib::proveedores::archivos::Indice::abrir(
        &contrato::directorio_del_indice(&base),
    )
    .expect("abrir lo recién escrito");

    let encontrados = indice.buscar("memoria", 10);
    assert_eq!(encontrados.len(), 1, "lo escrito tiene que poder buscarse");
    assert!(encontrados[0].id.ends_with("memoria anual.odt"));

    // Y el estado, que es lo que el gestor lee para saber si puede confiar en
    // que la ausencia sea ausencia.
    let estado = escaneo::leer_estado(&base).expect("tiene que haber estado");
    assert_eq!(
        estado.scan_state.as_deref(),
        Some(contrato::ESTADO_COMPLETO)
    );
    assert_eq!(estado.indexed_item_count, 1);

    let _ = std::fs::remove_dir_all(&base);
}

/// Sin esto, la prueba de arriba podría no estar comprobando nada.
#[test]
fn la_ruta_del_indice_es_la_que_el_gestor_ya_usa() {
    // Clavada contra la ruta real del sistema y no contra la función que la
    // arma, que se comprobaría a sí misma.
    let base = PathBuf::from("/home/quien-sea/.cache/vasak");
    assert_eq!(
        contrato::directorio_del_indice(&base),
        PathBuf::from("/home/quien-sea/.cache/vasak/global-search/v1/index")
    );
    assert_eq!(
        contrato::archivo_de_estado(&base),
        PathBuf::from("/home/quien-sea/.cache/vasak/global-search/status.json")
    );
}
