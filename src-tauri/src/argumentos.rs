//! Qué hacer según cómo se invocó al programa.
//!
//! Separado del arranque y sin tocar nada de afuera, que es lo que permite
//! probarlo: una invocación mal leída deja el atajo del teclado sin hacer nada,
//! y eso no falla en ningún lado.

/// Lo que el programa tiene que hacer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invocacion {
    /// Quedarse corriendo con la ventana escondida. Es como lo arranca la
    /// sesión, y también como lo arranca D-Bus al activarlo.
    Daemon,
    /// Mostrar u ocultar la ventana. Si no hay nadie corriendo, arrancar y
    /// mostrarla: es lo que hace que el atajo funcione la primera vez, antes de
    /// que la sesión haya levantado el servicio.
    Alternar,
    /// Decir qué versión es y salir.
    Version,
}

pub fn leer(args: &[String]) -> Invocacion {
    for arg in args {
        match arg.as_str() {
            "--daemon" => return Invocacion::Daemon,
            "--version" | "-v" => return Invocacion::Version,
            _ => {}
        }
    }

    // Sin argumentos también alterna: es lo que pasa al abrir la entrada del
    // escritorio o al escribir el nombre en una terminal, y abrir el lanzador
    // es lo que uno espera.
    Invocacion::Alternar
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leer_de(args: &[&str]) -> Invocacion {
        leer(&args.iter().map(|a| a.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn sin_argumentos_se_alterna() {
        assert_eq!(leer_de(&[]), Invocacion::Alternar);
    }

    #[test]
    fn el_daemon_se_pide_explicito() {
        // Lo usan la unidad de systemd y la activación por D-Bus. Si arrancara
        // alternando, la sesión abriría el lanzador en la cara al iniciar.
        assert_eq!(leer_de(&["--daemon"]), Invocacion::Daemon);
    }

    #[test]
    fn alternar_se_puede_pedir_por_su_nombre() {
        assert_eq!(leer_de(&["--toggle"]), Invocacion::Alternar);
    }

    #[test]
    fn la_version_se_pide_de_las_dos_formas() {
        assert_eq!(leer_de(&["--version"]), Invocacion::Version);
        assert_eq!(leer_de(&["-v"]), Invocacion::Version);
    }

    #[test]
    fn un_argumento_que_no_se_entiende_no_cambia_nada() {
        // Un lanzador que no abre porque le sobró una palabra es peor que uno
        // que la ignora.
        assert_eq!(leer_de(&["--qué-es-esto"]), Invocacion::Alternar);
    }

    #[test]
    fn el_daemon_gana_aunque_venga_al_final() {
        assert_eq!(leer_de(&["--algo", "--daemon"]), Invocacion::Daemon);
    }
}
