//! Armar y lanzar el comando de una entrada del escritorio.
//!
//! Dos cosas que la versión del escritorio hacía mal y se notaban al usarla:
//!
//!  1. **El hijo colgaba del proceso que lo lanzó.** Cerrar el lanzador se
//!     llevaba puesta la aplicación, y todo terminaba en el mismo cgroup: lo que
//!     consume Firefox se le contaba al lanzador, y un programa que se vuelve
//!     loco arrastraba a la ventana de búsqueda con él.
//!  2. **`Terminal=true` se ignoraba.** La entrada se lanzaba sin terminal: el
//!     proceso arranca, escribe en una salida que no existe y termina. Para
//!     quien lo eligió, no pasó nada.

use std::process::{Command, Stdio};

use crate::catalogo::exec;

/// La terminal del escritorio, para las entradas que piden una.
const TERMINAL: &str = "vasak-terminal";

/// Cómo se le pasa el programa: `vasak-terminal -e programa argumentos`.
const TERMINAL_EJECUTAR: &str = "-e";

/// El corralito de systemd donde vive lo que se lanza.
const SCOPE: &str = "systemd-run";

/// Arma el comando final de una entrada, listo para ejecutar.
///
/// `en_scope` dice si se envuelve en un ámbito de systemd. Se pasa y no se
/// decide acá para poder probar las dos formas sin systemd en la máquina.
pub fn armar(
    linea_exec: &str,
    nombre: &str,
    icono: Option<&str>,
    ruta: &str,
    terminal: bool,
    en_scope: bool,
) -> Option<Vec<String>> {
    let mut argumentos = exec::desarmar(linea_exec, nombre, icono, ruta)?;

    if terminal {
        let mut con_terminal = vec![TERMINAL.to_string(), TERMINAL_EJECUTAR.to_string()];
        con_terminal.append(&mut argumentos);
        argumentos = con_terminal;
    }

    if en_scope {
        argumentos = envolver_en_scope(argumentos);
    }

    Some(argumentos)
}

/// Envuelve el comando en un ámbito transitorio de systemd.
///
/// Con esto el proceso deja de ser hijo del lanzador: systemd lo adopta, le da
/// su propio cgroup y sobrevive a que la ventana se cierre. Es lo que hace GNOME
/// desde hace años y por los mismos motivos.
///
/// `--collect` limpia el ámbito cuando termina, aunque termine mal; sin eso, un
/// programa que se cae deja el ámbito en estado fallido y queda ahí hasta que
/// alguien lo saque a mano.
pub fn envolver_en_scope(argumentos: Vec<String>) -> Vec<String> {
    let mut salida = vec![
        SCOPE.to_string(),
        "--user".to_string(),
        "--scope".to_string(),
        "--quiet".to_string(),
        "--collect".to_string(),
        // La misma rebanada donde el sistema pone las aplicaciones del usuario,
        // que es lo que hace que los límites de recursos de la sesión valgan
        // también para lo que se lanza desde acá.
        "--slice=app.slice".to_string(),
        // El `--` es necesario: sin él, un `--modo` del programa lo lee
        // `systemd-run` como suyo y falla sin lanzar nada.
        "--".to_string(),
    ];
    salida.extend(argumentos);
    salida
}

/// Si en esta máquina se puede usar el ámbito de systemd.
///
/// Se comprueba una vez y no en cada lanzamiento. Sin systemd —un contenedor,
/// otra distribución— se lanza igual, directo: se pierde el cgroup propio, no la
/// posibilidad de abrir el programa.
pub fn hay_scope() -> bool {
    std::env::var_os("PATH")
        .map(|rutas| std::env::split_paths(&rutas).any(|dir| dir.join(SCOPE).exists()))
        .unwrap_or(false)
}

/// Lanza el comando y se desentiende.
///
/// El hilo que espera al hijo está para que no quede un zombi cuando no hay
/// systemd de por medio: un hijo que nadie espera se queda en la tabla de
/// procesos hasta que el padre termina, y el padre acá es un daemon que no
/// termina nunca.
pub fn lanzar(argumentos: &[String]) -> Result<(), String> {
    let (programa, resto) = argumentos.split_first().ok_or("comando vacío")?;

    let hijo = Command::new(programa)
        .args(resto)
        // Sin esto, lo que el programa escriba sale por la salida del lanzador y
        // termina en su diario, mezclado con lo suyo.
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("no se pudo lanzar {programa}: {error}"))?;

    std::thread::spawn(move || {
        let mut hijo = hijo;
        let _ = hijo.wait();
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(exec: &str, terminal: bool, en_scope: bool) -> Option<Vec<String>> {
        armar(exec, "Programa", None, "/x.desktop", terminal, en_scope)
    }

    #[test]
    fn un_comando_normal_se_arma_tal_cual() {
        assert_eq!(
            simple("firefox --new-window", false, false),
            Some(vec!["firefox".into(), "--new-window".into()])
        );
    }

    #[test]
    fn una_entrada_de_terminal_abre_una_terminal() {
        // Sin esto el programa arranca, escribe en una salida que no existe y
        // termina: para quien lo eligió, no pasó nada.
        assert_eq!(
            simple("htop", true, false),
            Some(vec!["vasak-terminal".into(), "-e".into(), "htop".into()])
        );
    }

    #[test]
    fn y_la_terminal_recibe_los_argumentos_del_programa() {
        assert_eq!(
            simple("journalctl -f", true, false),
            Some(vec![
                "vasak-terminal".into(),
                "-e".into(),
                "journalctl".into(),
                "-f".into()
            ])
        );
    }

    #[test]
    fn el_scope_envuelve_al_comando() {
        let armado = simple("firefox", false, true).unwrap();

        assert_eq!(armado[0], "systemd-run");
        assert!(armado.contains(&"--user".to_string()));
        assert!(armado.contains(&"--scope".to_string()));
        assert!(armado.contains(&"--collect".to_string()));
        assert_eq!(armado.last().unwrap(), "firefox");
    }

    #[test]
    fn el_doble_guion_separa_lo_de_systemd_de_lo_del_programa() {
        // Sin él, un `--private-window` del programa lo lee `systemd-run` como
        // suyo, no lo entiende y no lanza nada.
        let armado = simple("firefox --private-window", false, true).unwrap();
        let corte = armado.iter().position(|arg| arg == "--").unwrap();

        assert_eq!(&armado[corte + 1..], &["firefox", "--private-window"]);
    }

    #[test]
    fn la_terminal_tambien_va_adentro_del_scope() {
        // Al revés —el scope adentro de la terminal— el corralito quedaría
        // alrededor del comando y no de la terminal que lo dibuja.
        let armado = simple("htop", true, true).unwrap();
        let corte = armado.iter().position(|arg| arg == "--").unwrap();

        assert_eq!(&armado[corte + 1..], &["vasak-terminal", "-e", "htop"]);
    }

    #[test]
    fn una_linea_que_no_se_puede_desarmar_no_se_lanza() {
        assert_eq!(simple("", false, false), None);
        assert_eq!(simple(r#"programa "sin cerrar"#, false, false), None);
    }

    #[test]
    fn lanzar_sin_comando_no_intenta_nada() {
        assert!(lanzar(&[]).is_err());
    }

    #[test]
    fn lanzar_algo_que_no_existe_lo_dice() {
        assert!(lanzar(&["no-existe-este-programa-en-ningun-lado".to_string()]).is_err());
    }

    #[test]
    fn lanzar_algo_que_existe_no_falla_ni_deja_zombis() {
        lanzar(&["true".to_string()]).expect("true tiene que poder lanzarse");
    }
}
