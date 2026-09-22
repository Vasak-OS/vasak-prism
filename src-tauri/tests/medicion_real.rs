//! Cuánto cuesta de verdad, con el código que se va a instalar.
//!
//! Se salta sola si no hay una cuenta que recorrer. No afirma tiempos —una
//! máquina de CI no es la de nadie— sino que imprime lo medido para que el
//! número del que dependen las decisiones de `escaneo` se pueda volver a sacar.
#[test]
#[ignore = "recorre el disco entero; se corre a mano con --ignored"]
fn cuanto_cuesta_un_escaneo_entero() {
    let base = std::env::temp_dir().join(format!("prism-medicion-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);

    let raices = vasak_prism_lib::proveedores::archivos::escaneo::raices();
    let ignoradas: Vec<String> = vasak_prism_lib::proveedores::archivos::escaneo::ignoradas_por_omision()
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    let arranque = std::time::Instant::now();
    let cuantas = vasak_prism_lib::proveedores::archivos::escaneo::escanear(
        &base,
        &raices,
        &ignoradas,
        &std::sync::atomic::AtomicBool::new(false),
        vasak_prism_lib::proveedores::archivos::escaneo::ahora_ms(),
    )
    .expect("escanear");
    let tardo = arranque.elapsed();

    let indice = vasak_prism_lib::proveedores::archivos::contrato::directorio_del_indice(&base);
    let bytes: u64 = std::fs::read_dir(&indice)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum();

    println!("raíces:     {raices:?}");
    println!("entradas:   {cuantas}");
    println!("tardó:      {:.1} s", tardo.as_secs_f64());
    println!("en disco:   {:.1} MB", bytes as f64 / 1e6);

    let _ = std::fs::remove_dir_all(&base);
}
