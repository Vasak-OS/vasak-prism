//! Enterarse de que se instaló o se borró una aplicación, sin sondear.
//!
//! Lo que había en el escritorio releía el catálogo **por reloj, cada cinco
//! minutos**: instalabas algo y podía tardar todo ese rato en aparecer, y
//! mientras tanto releía quinientos archivos seis veces por hora aunque no
//! hubiera cambiado nada. Las dos mitades están mal.
//!
//! Acá el disco avisa. El hilo se bloquea en el kernel y no despierta ni una vez
//! mientras no pasa nada.
//!
//! La espera de ráfaga es la misma idea que `inotify_rafaga` de `vasak-desktop`,
//! y está copiada a propósito: son cuarenta líneas y los dos repositorios no
//! comparten ningún crate. Si aparece un tercero, va a un lugar común.

use std::path::PathBuf;
use std::thread::JoinHandle;
use std::time::Duration;

use inotify::{Inotify, WatchMask};

/// Cuánto silencio hace falta para dar la ráfaga por terminada.
///
/// Instalar un paquete escribe muchos archivos seguidos; actualizar el sistema,
/// cientos. Reindexar por cada evento es rehacer el mismo trabajo decenas de
/// veces mientras el gestor de paquetes todavía está escribiendo.
pub const REPOSO: Duration = Duration::from_millis(400);

/// Lo que hay que mirar de un directorio de aplicaciones.
///
/// `CLOSE_WRITE` y no `MODIFY`: un archivo que se está escribiendo dispara
/// `MODIFY` varias veces y en el medio está incompleto. Los dos `MOVED` son el
/// caso habitual de los gestores de paquetes, que escriben aparte y renombran.
const EVENTOS: WatchMask = WatchMask::CREATE
    .union(WatchMask::DELETE)
    .union(WatchMask::CLOSE_WRITE)
    .union(WatchMask::MOVED_TO)
    .union(WatchMask::MOVED_FROM);

/// Vigila los directorios y llama a `al_cambiar` una vez por ráfaga.
///
/// Devuelve el hilo. Soltarlo no lo detiene: vive lo que vive el proceso, que
/// es exactamente lo que se quiere de un daemon.
pub fn vigilar(
    directorios: Vec<PathBuf>,
    reposo: Duration,
    al_cambiar: impl Fn() + Send + 'static,
) -> std::io::Result<JoinHandle<()>> {
    let mut inotify = Inotify::init()?;

    let mut vigilados = 0;
    for directorio in &directorios {
        // Un directorio que no existe no es un error: `~/.local/share/applications`
        // no está hasta que el usuario instala algo suyo.
        if inotify.watches().add(directorio, EVENTOS).is_ok() {
            vigilados += 1;
        }
    }

    if vigilados == 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "ningún directorio de aplicaciones para vigilar",
        ));
    }

    Ok(std::thread::spawn(move || {
        let mut buffer = [0u8; 4096];
        loop {
            match esperar_rafaga(&mut inotify, &mut buffer, reposo) {
                Ok(()) => al_cambiar(),
                // Un error de lectura acá deja el hilo dando vueltas a toda
                // velocidad si se sigue. Mejor dejar de vigilar: el índice queda
                // viejo hasta el próximo arranque, que es malo, pero no es un
                // núcleo al cien por ciento.
                Err(_) => return,
            }
        }
    }))
}

/// Espera a que haya cambios y a que dejen de haberlos.
///
/// Bloquea en el kernel hasta el primer evento y después drena hasta que pasa
/// `reposo` sin novedades. Mientras no pasa nada, el hilo no despierta.
fn esperar_rafaga(
    inotify: &mut Inotify,
    buffer: &mut [u8],
    reposo: Duration,
) -> std::io::Result<()> {
    if inotify.read_events_blocking(buffer)?.count() == 0 {
        return Ok(());
    }

    loop {
        std::thread::sleep(reposo);

        match inotify.read_events(buffer) {
            Ok(eventos) => {
                if eventos.count() == 0 {
                    return Ok(());
                }
            }
            // Sin eventos pendientes es la señal de que la ráfaga se apagó, no
            // un error.
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(()),
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::sync::Arc;

    fn directorio(nombre: &str) -> PathBuf {
        let ruta =
            std::env::temp_dir().join(format!("prism-vigilancia-{}-{nombre}", std::process::id()));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(&ruta).expect("crear el directorio de prueba");
        ruta
    }

    #[test]
    fn una_rafaga_avisa_una_sola_vez() {
        // El punto del reposo: instalar un paquete escribe muchos archivos
        // seguidos y reindexar por cada uno es rehacer lo mismo decenas de veces.
        let dir = directorio("rafaga");
        let veces = Arc::new(AtomicUsize::new(0));
        let (aviso, recibo) = mpsc::channel();

        let contador = Arc::clone(&veces);
        vigilar(vec![dir.clone()], Duration::from_millis(150), move || {
            contador.fetch_add(1, Ordering::SeqCst);
            let _ = aviso.send(());
        })
        .expect("vigilar");

        for i in 0..5 {
            std::fs::write(dir.join(format!("app-{i}.desktop")), b"x").unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }

        recibo
            .recv_timeout(Duration::from_secs(3))
            .expect("tenía que avisar");
        // Un rato más, por si iba a avisar de nuevo por los mismos cinco.
        std::thread::sleep(Duration::from_millis(400));

        assert_eq!(veces.load(Ordering::SeqCst), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sin_cambios_no_avisa() {
        // Se comprueba por el negativo: en medio segundo sin tocar nada, nada.
        let dir = directorio("quieto");
        let (aviso, recibo) = mpsc::channel();

        vigilar(vec![dir.clone()], Duration::from_millis(50), move || {
            let _ = aviso.send(());
        })
        .expect("vigilar");

        assert!(
            recibo.recv_timeout(Duration::from_millis(600)).is_err(),
            "avisó sin que nada cambiara: eso es sondear, no escuchar"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn borrar_una_aplicacion_tambien_avisa() {
        // Desinstalar tiene que sacarla de los resultados; si no, queda una fila
        // que al elegirla no abre nada.
        let dir = directorio("borrado");
        std::fs::write(dir.join("app.desktop"), b"x").unwrap();

        let (aviso, recibo) = mpsc::channel();
        vigilar(vec![dir.clone()], Duration::from_millis(50), move || {
            let _ = aviso.send(());
        })
        .expect("vigilar");

        std::fs::remove_file(dir.join("app.desktop")).unwrap();

        assert!(recibo.recv_timeout(Duration::from_secs(3)).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn un_directorio_que_no_existe_no_impide_vigilar_los_demas() {
        // `~/.local/share/applications` no existe hasta que el usuario instala
        // algo suyo, y eso no puede dejar sin vigilancia a `/usr/share`.
        let dir = directorio("mezcla");
        let inexistente = dir.join("no-esta");

        assert!(vigilar(vec![inexistente, dir.clone()], REPOSO, || {}).is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sin_ningun_directorio_valido_avisa_del_problema() {
        let inexistente = std::env::temp_dir().join("prism-no-existe-nada-aca");
        assert!(vigilar(vec![inexistente], REPOSO, || {}).is_err());
    }
}
