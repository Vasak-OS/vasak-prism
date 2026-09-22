//! Recorrer el disco y escribir el índice, que desde la 0.13 es trabajo de acá.
//!
//! # Por qué se mudó
//!
//! Lo escribía el gestor de archivos, y sólo cuando alguien abría su ventana y
//! lo pedía. Eso alcanza para el gestor —lo abrís, buscás, y si hace falta
//! escaneás— y no alcanza para un lanzador: un archivo que bajaste hace diez
//! minutos no está, y nada te dice por qué. El lanzador es el que vive
//! prendido, así que es el que puede mantenerlo al día. Ver
//! Vasak-OS/vasak-prism#33.
//!
//! # Cuánto cuesta, medido y no estimado
//!
//! Sobre una cuenta real, con este mismo código compilado en release:
//! **78.500 entradas, 4,1 segundos y 9,3 MB de índice**, con menos de 60 MB de
//! pico de memoria. Es barato, y eso es lo que decide el resto del diseño: no
//! hace falta ser incremental para ser útil, alcanza con rehacerlo entero
//! cuando está viejo.
//!
//! **Y si alguien lo vuelve a medir, que sea en release.** El mismo escaneo en
//! depuración tarda 17,1 segundos —cuatro veces más— porque lo caro es tantivy
//! comprimiendo, y sin optimizar eso se nota entero. Medirlo con `cargo test`
//! a secas y sacar conclusiones de ahí llevaría a rediseñar esto por un número
//! que ninguna instalación va a ver. La medición vive en
//! `tests/medicion_real.rs` y se corre con `--release`.
//!
//! # Cuándo escanea
//!
//! **Cuando se abre el lanzador y lo que hay está viejo**, nunca al arrancar la
//! sesión y nunca por reloj. Los tres se pensaron con el número de arriba en la
//! mano:
//!
//! - *Al arrancar la sesión* es el peor momento: es cuando todo lo demás
//!   arranca, y son cuatro segundos de disco compitiendo con el escritorio
//!   entero por algo que nadie pidió todavía.
//! - *Por reloj, cada tantos minutos* mantiene el índice fresco en una máquina
//!   que nadie está usando, y cuesta reescribir 9,4 MB cada vez, para siempre.
//!   En un portátil eso es batería y desgaste a cambio de nada.
//! - *Al abrir el lanzador* no cuesta nada cuando nadie lo abre, y cuando se
//!   abre seguido el índice ya está fresco. Es además el patrón que este mismo
//!   programa ya usa para las cotizaciones: se contesta con lo que hay y se
//!   refresca para la próxima.
//!
//! **Lo que ese último eligió, dicho de frente:** la búsqueda que dispara el
//! escaneo se contesta con el índice viejo. Si bajaste un archivo y abrís el
//! lanzador por primera vez en media hora, esa primera búsqueda no lo
//! encuentra; la siguiente sí, unos segundos después. Lo que lo arregla del
//! todo es vigilar con inotify e ir actualizando de a un archivo, y eso es un
//! paso aparte —está medido que entra: 10.636 directorios contra un límite de
//! 524.288— que no hace falta para cambiar de dueño, que es de lo que se trata
//! este.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tantivy::{doc, Index, IndexWriter};
use walkdir::WalkDir;

use super::contrato;

/// Cuánto vale un índice antes de convenir rehacerlo.
///
/// Quince minutos sale del caso que motiva todo esto —«bajé un archivo hace
/// diez minutos y no aparece»— y no de un número redondo: por debajo de eso el
/// archivo recién bajado ya está, y por encima empieza a pasar justo lo que se
/// vino a arreglar.
pub const VIGENCIA_MS: u64 = 15 * 60 * 1000;

/// Cuánto vale un «en curso» antes de darlo por un escaneo que murió.
///
/// Cinco minutos, y el número sale de la medición: un escaneo entero tarda
/// segundos, así que esto ya es setenta y cinco veces lo que tarda. El gestor
/// no tiene esta constante escrita: la lee del archivo, así que bajarla acá la
/// baja para los dos. Lo que no se puede es subirla sin motivo — un escaneo que
/// se muere de golpe bloquea el siguiente todo este rato.
pub const VENCE_EN_CURSO_MS: u64 = 5 * 60 * 1000;

/// Hasta dónde baja el recorrido desde cada raíz.
///
/// Cinco, que es lo que el gestor venía usando. Bajarlo pierde archivos y
/// subirlo cuesta: la mayor parte de lo que hay más abajo en una cuenta real
/// son dependencias y objetos de compilación, que ya están excluidos por
/// nombre pero cuyos directorios igual habría que recorrer para descartarlos.
pub const PROFUNDIDAD: usize = 5;

/// Lo que no se indexa nunca.
///
/// Dos clases, y la diferencia importa: las que **empiezan con barra** son
/// rutas del sistema y cuentan sólo desde la raíz; las demás son **nombres de
/// directorio** y cuentan en cualquier nivel.
///
/// Mezclarlas es un error con consecuencias: `node_modules` tiene que salir
/// esté donde esté, y `/dev` tiene que salir sólo si es el `/dev` del sistema.
/// Tratando a las dos igual, un `~/proyectos/dev` desaparece del índice entero
/// —y también `~/tmp`, `~/run` y cualquier `Documentos/sys`—. No falla, no
/// avisa: esos archivos simplemente no aparecen nunca al buscar.
///
/// No son sólo directorios del sistema: `node_modules`, `target` y `.git` son
/// la diferencia entre 78 mil entradas y 498 mil en una cuenta con proyectos,
/// medido en la misma máquina — o sea que más de la mitad del disco de quien
/// programa es ruido que nadie busca por nombre.
pub fn ignoradas_por_omision() -> &'static [&'static str] {
    &[
        // Rutas del sistema: desde la raíz y en ningún otro lado.
        "/proc",
        "/sys",
        "/dev",
        "/run",
        "/tmp",
        "/var/tmp",
        "/lost+found",
        "/$Recycle.Bin",
        "/System Volume Information",
        // Nombres de directorio: en cualquier nivel.
        "node_modules",
        ".git",
        "target",
        ".cache",
        ".Trash",
        ".Trashes",
        ".Spotlight-V100",
        ".fseventsd",
        "Library/Caches",
        "AppData/Local/Temp",
    ]
}

/// Si una ruta cae en la lista de ignoradas.
///
/// - Lo que **empieza con barra** es una ruta absoluta y coincide sólo desde la
///   raíz: `/dev` saca `/dev/null` y no saca `~/proyectos/dev`. Es también cómo
///   se escribe una carpeta propia que uno no quiere indexar.
/// - Lo demás es un **nombre de directorio** y coincide en cualquier nivel,
///   entero: `target` saca `~/a/b/target` y no saca `~/mi-target-viejo`. Puede
///   tener varios segmentos —`Library/Caches`— y entonces tienen que aparecer
///   seguidos.
///
/// «Entero» es la parte que hay que cuidar: comparar por prefijo o por
/// subcadena suelta se lleva puesto todo lo que empiece igual, y eso se ve como
/// archivos que no aparecen, nunca como un error.
pub fn es_ignorada(ruta: &str, ignoradas: &[String]) -> bool {
    ignoradas.iter().any(|ignorada| {
        let limpia = ignorada.trim().trim_end_matches('/');
        if limpia.is_empty() {
            return false;
        }

        if limpia.starts_with('/') {
            return ruta == limpia || ruta.starts_with(&format!("{limpia}/"));
        }

        ruta.contains(&format!("/{limpia}/")) || ruta.ends_with(&format!("/{limpia}"))
    })
}

/// Las reglas que valen adentro de una raíz.
///
/// Una regla absoluta que **contiene** a la raíz no puede aplicarse ahí: la
/// raíz se está recorriendo porque alguien la pidió, así que excluirla por una
/// regla que habla de un directorio de más arriba es desobedecer lo pedido.
///
/// No es teórico y por poco se va instalado: `/run` está en la lista —es el
/// `/run` del sistema, lleno de sockets y de archivos de estado— y en Arch y en
/// Fedora udisks2 monta lo que se enchufa en `/run/media/$USER/`. Con la regla
/// aplicada a secas, **ningún pendrive se indexaba nunca**, y sin ningún aviso:
/// el recorrido se podaba en la raíz misma.
///
/// Las reglas por nombre —`node_modules`, `.git`— no entran en esto: valen en
/// cualquier nivel justamente porque no hablan de un lugar.
fn reglas_para(raiz: &Path, ignoradas: &[String]) -> Vec<String> {
    ignoradas
        .iter()
        .filter(|regla| {
            let limpia = regla.trim().trim_end_matches('/');
            if !limpia.starts_with('/') {
                return true;
            }
            !(raiz == Path::new(limpia) || raiz.starts_with(format!("{limpia}/")))
        })
        .cloned()
        .collect()
}

/// Desde dónde se recorre.
///
/// La carpeta del usuario y lo que haya montado en los lugares de siempre. No
/// es el disco entero a propósito: `/usr` y `/var` son del sistema, cambian con
/// cada actualización y nadie busca ahí por nombre desde un lanzador.
pub fn raices() -> Vec<PathBuf> {
    raices_desde(
        dirs::home_dir(),
        &std::fs::read_to_string("/proc/mounts").unwrap_or_default(),
    )
}

/// Los lugares donde el escritorio monta lo que se enchufa.
const DONDE_SE_MONTA: &[&str] = &["/run/media/", "/media/", "/mnt/"];

/// Lo mismo, recibiendo lo que hay que leer en vez de leerlo.
///
/// Aparte para poder probarlo: `/proc/mounts` es distinto en cada máquina y en
/// cada momento, así que una prueba que lo lea de verdad comprueba la máquina y
/// no el código.
pub fn raices_desde(hogar: Option<PathBuf>, montajes: &str) -> Vec<PathBuf> {
    let mut raices = Vec::new();

    // La base sale de `dirs` pero igual se filtra: `dirs` mira que `HOME` no
    // esté vacío y no que sea absoluto, y un recorrido desde una ruta relativa
    // arrancaría en el directorio de trabajo del daemon.
    if let Some(hogar) = hogar.filter(|ruta| ruta.is_absolute()) {
        raices.push(hogar);
    }

    for linea in montajes.lines() {
        // `/proc/mounts` es «dispositivo punto-de-montaje tipo opciones …», y
        // el punto de montaje trae los espacios escapados como `\040`.
        let Some(punto) = linea.split_whitespace().nth(1) else {
            continue;
        };
        let punto = punto.replace("\\040", " ");

        if DONDE_SE_MONTA.iter().any(|donde| punto.starts_with(donde)) {
            let punto = PathBuf::from(punto);
            if !raices.contains(&punto) {
                raices.push(punto);
            }
        }
    }

    raices
}

/// Lo que se sabe del último escaneo.
///
/// **Los nombres de los campos son del contrato**: este archivo lo escribe el
/// lanzador y lo lee el gestor, que los tiene escritos a mano de su lado.
/// Renombrar uno acá no rompe nada visible, sólo deja al otro sin saber.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Estado {
    pub last_scan_time: Option<u64>,
    pub indexed_item_count: u64,
    pub schema_version: u32,
    /// Todos opcionales porque este archivo lo escribieron versiones que no los
    /// tenían, y uno viejo tiene que poder leerse **entero**: si el parseo
    /// fallara se perderían también la versión y la fecha, que sí están.
    #[serde(default)]
    pub scan_state: Option<String>,
    #[serde(default)]
    pub scan_state_time: Option<u64>,
    #[serde(default)]
    pub scan_state_ttl_ms: Option<u64>,
}

/// Lo que haya escrito, o nada.
pub fn leer_estado(base: &Path) -> Option<Estado> {
    let texto = std::fs::read_to_string(contrato::archivo_de_estado(base)).ok()?;
    serde_json::from_str(&texto).ok()
}

/// Deja escrito en qué quedó el escaneo, sin pisar lo que no toca.
///
/// Lee lo que había para no perder la fecha ni el conteo cuando el escaneo
/// falla antes de tener uno nuevo: un fallo no puede además borrar lo poco que
/// se sabía.
pub fn escribir_estado(base: &Path, estado: &str, conteo: Option<u64>, ahora: u64) {
    let previo = leer_estado(base);

    let nuevo = Estado {
        last_scan_time: if conteo.is_some() {
            Some(ahora)
        } else {
            previo.as_ref().and_then(|p| p.last_scan_time)
        },
        indexed_item_count: conteo.unwrap_or_else(|| {
            previo
                .as_ref()
                .map(|p| p.indexed_item_count)
                .unwrap_or_default()
        }),
        schema_version: contrato::VERSION,
        scan_state: Some(estado.to_string()),
        scan_state_time: Some(ahora),
        scan_state_ttl_ms: Some(VENCE_EN_CURSO_MS),
    };

    let ruta = contrato::archivo_de_estado(base);
    if let Some(padre) = ruta.parent() {
        if std::fs::create_dir_all(padre).is_err() {
            return;
        }
    }
    if let Ok(texto) = serde_json::to_string(&nuevo) {
        // Al lado y renombrado encima: escribir directo trunca primero, y un
        // corte ahí deja un `status.json` a medias, que se lee como que no hay
        // ninguno. El gestor perdería la fecha y el conteo.
        let provisorio = ruta.with_extension("json.nuevo");
        if std::fs::write(&provisorio, texto).is_ok()
            && std::fs::rename(&provisorio, &ruta).is_err()
        {
            let _ = std::fs::remove_file(&provisorio);
        }
    }
}

/// Si conviene rehacer el índice.
///
/// Aparte del escaneo y sin tocar nada, que es lo que permite probar la
/// decisión entera sin disco: son cuatro casos y tres de ellos no se pueden
/// provocar a mano con comodidad.
pub fn conviene_escanear(estado: Option<&Estado>, ahora: u64, vigencia: u64) -> bool {
    let Some(estado) = estado else {
        // Nunca se escaneó, o lo escrito no se entiende. Las dos se arreglan
        // igual.
        return true;
    };

    // Hay uno corriendo y todavía vale: puede ser de este proceso o del gestor
    // de archivos sin actualizar, y en los dos casos meter un segundo escritor
    // no aporta nada.
    if estado.scan_state.as_deref() == Some(contrato::ESTADO_EN_CURSO) {
        let escrito = estado.scan_state_time.unwrap_or(0);
        let vence = estado.scan_state_ttl_ms.unwrap_or(VENCE_EN_CURSO_MS);
        if contrato::en_curso_sigue_vivo(escrito, vence, ahora) {
            return false;
        }
    }

    // Si el esquema guardado no es el que este código sabe escribir, hay que
    // rehacerlo sí o sí: el índice de otra versión no se puede completar.
    if estado.schema_version != contrato::VERSION {
        return true;
    }

    match estado.last_scan_time {
        // Una fecha del futuro es un reloj corregido hacia atrás. Se rehace, que
        // cuesta segundos, en lugar de no volver a hacerlo nunca.
        Some(ultimo) if ultimo <= ahora => ahora - ultimo >= vigencia,
        Some(_) => true,
        None => true,
    }
}

/// El índice abierto para escribir, creándolo si no está.
///
/// Si lo que hay tiene otro esquema se descarta entero y se rehace: vaciarlo en
/// el lugar conservaría el esquema viejo, y el nuevo no se puede crear encima.
///
/// Es público para poder comprobar lo que ninguna prueba de unidad alcanza:
/// que un índice escrito por **el otro programa** se conserve en vez de
/// descartarse. Ver `tests/indice_de_archivos_real.rs`.
pub fn abrir_para_escribir(ruta: &Path) -> Result<Index, String> {
    std::fs::create_dir_all(ruta).map_err(|e| e.to_string())?;
    let (esquema, _) = contrato::esquema();

    match Index::open_in_dir(ruta) {
        Ok(existente) if existente.schema() == esquema => Ok(existente),
        Ok(existente) => {
            drop(existente);
            std::fs::remove_dir_all(ruta).map_err(|e| e.to_string())?;
            std::fs::create_dir_all(ruta).map_err(|e| e.to_string())?;
            Index::create_in_dir(ruta, esquema).map_err(|e| e.to_string())
        }
        Err(_) => Index::create_in_dir(ruta, esquema).map_err(|e| e.to_string()),
    }
}

/// Cuánta memoria le damos al escritor de tantivy.
///
/// Cincuenta megas, elegido midiendo: con quince tarda lo mismo y con
/// doscientos tampoco mejora, así que lo único que cambia es el pico de memoria
/// del daemon —42, 56 y 92 MB respectivamente—. Cincuenta es el punto donde
/// deja de comprarse nada con más.
const MEMORIA_DEL_ESCRITOR: usize = 50_000_000;

/// Mete una entrada del disco en el índice.
///
/// Lo que no se puede leer se saltea sin ruido: entre 78 mil archivos siempre
/// hay alguno que desapareció entre que el recorrido lo vio y esto lo miró.
fn documentar(escritor: &mut IndexWriter, campos: &contrato::Campos, ruta: &Path) {
    let Ok(datos) = std::fs::metadata(ruta) else {
        return;
    };
    let (Some(nombre), Some(ruta_texto)) =
        (ruta.file_name().and_then(|n| n.to_str()), ruta.to_str())
    else {
        return;
    };

    let es_directorio = datos.is_dir();
    let es_archivo = datos.is_file();
    let modificado = datos
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);

    let _ = escritor.add_document(doc!(
        campos.ruta => ruta_texto,
        campos.nombre => nombre,
        campos.nombre_minuscula => nombre.to_lowercase(),
        campos.es_archivo => u64::from(es_archivo),
        campos.es_directorio => u64::from(es_directorio),
        campos.modificado => modificado,
        campos.tamanio => if es_archivo { datos.len() } else { 0 },
    ));
}

/// Recorre las raíces y rehace el índice entero.
///
/// Devuelve cuántas entradas quedaron indexadas.
///
/// # Que no se pueda escribir no es un error de acá
///
/// Tantivy admite **un solo escritor**, y se lo asegura con un archivo de
/// bloqueo. Mientras el gestor de archivos sin actualizar siga escaneando,
/// puede tener el suyo tomado: ahí esto devuelve `Err` y el que llama no hace
/// nada. No es una falla, es el otro haciendo el mismo trabajo — y es lo que
/// hace que el cambio de dueño no necesite que las dos aplicaciones se
/// actualicen en el mismo instante.
pub fn escanear(
    base: &Path,
    raices: &[PathBuf],
    ignoradas: &[String],
    cancelar: &AtomicBool,
    ahora: u64,
) -> Result<u64, String> {
    let ruta = contrato::directorio_del_indice(base);
    let indice = abrir_para_escribir(&ruta)?;
    let (_, campos) = contrato::esquema();

    let mut escritor: IndexWriter = indice
        .writer(MEMORIA_DEL_ESCRITOR)
        .map_err(|e| format!("ya hay alguien escribiendo el índice: {e}"))?;

    escribir_estado(base, contrato::ESTADO_EN_CURSO, None, ahora);

    // Se vacía y se llena en la misma transacción: hasta el `commit` los
    // lectores siguen viendo el índice anterior entero. Sin eso habría un rato
    // —los segundos que tarda— en que la búsqueda de archivos no encuentra nada.
    escritor.delete_all_documents().map_err(|e| e.to_string())?;

    let mut contadas: u64 = 0;
    let mut cancelado = false;

    for raiz in raices {
        if cancelado {
            break;
        }
        // Por raíz y no una vez: lo que excluye a una puede ser justamente lo
        // que otra pidió recorrer.
        let reglas = reglas_para(raiz, ignoradas);
        for entrada in WalkDir::new(raiz)
            .follow_links(false)
            .max_depth(PROFUNDIDAD.max(1))
            .into_iter()
            .filter_entry(|e| !es_ignorada(&e.path().to_string_lossy(), &reglas))
        {
            if cancelar.load(Ordering::SeqCst) {
                cancelado = true;
                break;
            }
            let Ok(entrada) = entrada else { continue };
            // La raíz misma no se indexa: es el directorio desde el que se mira.
            if entrada.depth() == 0 {
                continue;
            }
            documentar(&mut escritor, &campos, entrada.path());
            contadas += 1;
        }
    }

    if cancelado {
        // Lo recorrido hasta acá se guarda igual: es menos que todo, y el
        // estado lo dice. Tirarlo dejaría al lanzador sin archivos por haber
        // cancelado, que es peor que tener una parte.
        escritor.commit().map_err(|e| e.to_string())?;
        escribir_estado(base, contrato::ESTADO_CANCELADO, Some(contadas), ahora);
        return Ok(contadas);
    }

    escritor.commit().map_err(|e| e.to_string())?;
    escribir_estado(base, contrato::ESTADO_COMPLETO, Some(contadas), ahora);
    Ok(contadas)
}

/// Ahora, en milisegundos desde la época.
pub fn ahora_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Quién decide si se escanea, y se asegura de que no haya dos a la vez.
pub struct Escaneos {
    base: Option<PathBuf>,
    /// Si este proceso ya tiene uno corriendo.
    ///
    /// Aparte del `status.json`: aquél lo comparten los dos programas y se lee
    /// del disco, así que entre que se lee y se arranca hay una ventana. Esto
    /// cierra la de acá, que es la que se puede dar seguido — abrir el lanzador
    /// dos veces en tres segundos.
    corriendo: Arc<AtomicBool>,
}

impl Escaneos {
    pub fn nuevos(base: Option<PathBuf>) -> Self {
        Self {
            base,
            corriendo: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Escanea en otro hilo si hace falta. No espera nada.
    ///
    /// Se llama desde donde se atiende una consulta, así que no puede tardar:
    /// lo único que hace en el hilo de quien llama es leer un archivo chico y
    /// decidir.
    pub fn refrescar_si_conviene(&self) {
        let Some(base) = self.base.clone() else {
            return;
        };

        let ahora = ahora_ms();
        if !conviene_escanear(leer_estado(&base).as_ref(), ahora, VIGENCIA_MS) {
            return;
        }

        // `swap` y no `load` + `store`: dos consultas seguidas entran acá a la
        // vez, y con dos operaciones las dos se verían libres.
        if self.corriendo.swap(true, Ordering::SeqCst) {
            return;
        }

        let corriendo = Arc::clone(&self.corriendo);
        std::thread::spawn(move || {
            let sin_cancelar = AtomicBool::new(false);
            let ignoradas: Vec<String> = ignoradas_por_omision()
                .iter()
                .map(|s| (*s).to_string())
                .collect();

            if let Err(_error) = escanear(&base, &raices(), &ignoradas, &sin_cancelar, ahora_ms()) {
                // No se pudo tomar el índice: lo tiene el gestor sin
                // actualizar, o el disco no deja escribir. Se reintenta la
                // próxima vez que alguien abra el lanzador.
                escribir_estado(&base, contrato::ESTADO_FALLADO, None, ahora_ms());
            }

            corriendo.store(false, Ordering::SeqCst);
        });
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn ignoradas() -> Vec<String> {
        ignoradas_por_omision()
            .iter()
            .map(|s| (*s).to_string())
            .collect()
    }

    /// Lo que se le pasa a las pruebas que escanean un árbol de mentira.
    ///
    /// La lista de omisión no sirve acá: el árbol se arma en el directorio
    /// temporal, que cuelga de `/tmp`, y `/tmp` es de las rutas del sistema que
    /// no se indexan. Con ella no se indexaría nada y las pruebas pasarían por
    /// el motivo equivocado.
    fn solo_target() -> Vec<String> {
        vec!["target".to_string()]
    }

    /// Un directorio propio de esta prueba, que no choca con las demás.
    ///
    /// Las pruebas corren en paralelo y todas escriben índices: una ruta
    /// compartida las hace fallar al azar, y de a una por vez pasan.
    fn directorio(nombre: &str) -> PathBuf {
        let ruta = std::env::temp_dir().join(format!(
            "prism-escaneo-{nombre}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&ruta);
        std::fs::create_dir_all(&ruta).unwrap();
        ruta
    }

    #[test]
    fn un_nombre_de_directorio_se_ignora_en_cualquier_nivel() {
        let lista = ignoradas();

        assert!(es_ignorada("/home/pato/proyecto/target/deps", &lista));
        assert!(es_ignorada("/home/pato/a/b/c/node_modules", &lista));
        assert!(es_ignorada("/home/pato/proyecto/.git", &lista));
        assert!(es_ignorada("/home/pato/Library/Caches/loquesea", &lista));
    }

    #[test]
    fn una_ruta_del_sistema_se_ignora_solo_desde_la_raiz() {
        // El agujero que esto cierra: tratando `/dev` como un segmento que vale
        // en cualquier nivel, `~/proyectos/dev` desaparecía del índice entero.
        // Y con él `~/tmp`, `~/run` y cualquier `Documentos/sys`. No fallaba y
        // no avisaba: esos archivos simplemente no aparecían nunca al buscar.
        let lista = ignoradas();

        assert!(es_ignorada("/dev/null", &lista), "el /dev del sistema sí");
        assert!(es_ignorada("/proc/1/status", &lista));
        assert!(es_ignorada("/tmp/lo-que-sea", &lista));

        for propia in [
            "/home/pato/proyectos/dev/main.rs",
            "/home/pato/tmp/borrador.txt",
            "/home/pato/Documentos/sys/notas.md",
            "/home/pato/run/informe.pdf",
        ] {
            assert!(
                !es_ignorada(propia, &lista),
                "«{propia}» es del usuario, no del sistema"
            );
        }
    }

    #[test]
    fn un_nombre_que_empieza_igual_no_se_ignora() {
        // Es el error que la comparación por prefijo cometería: `/target`
        // tiene que sacar el directorio `target` y no todo lo que empiece con
        // esas letras. Quien tenga `~/targets-2026` lo perdería entero.
        let lista = ignoradas();

        assert!(!es_ignorada("/home/pato/mi-target-viejo", &lista));
        assert!(!es_ignorada("/home/pato/targets-2026/informe.pdf", &lista));
        assert!(!es_ignorada("/home/pato/.gitignore", &lista));
    }

    #[test]
    fn una_carpeta_propia_se_saca_escribiendola_entera() {
        // Quien no quiere que se indexe `~/Privado` pone la ruta absoluta, y
        // entonces vale para ésa y no para cualquier «Privado» del disco.
        let lista = vec!["/home/pato/Privado".to_string()];

        assert!(es_ignorada("/home/pato/Privado", &lista));
        assert!(es_ignorada("/home/pato/Privado/carta.txt", &lista));
        assert!(!es_ignorada("/home/otro/Privado/carta.txt", &lista));
        assert!(
            !es_ignorada("/home/pato/Privadores/x", &lista),
            "empezar igual no alcanza"
        );
    }

    #[test]
    fn una_ignorada_vacia_no_saca_todo() {
        // Una cadena vacía es prefijo de cualquier ruta, así que sin este caso
        // una línea en blanco en la configuración dejaría el índice sin nada.
        assert!(!es_ignorada("/home/pato/cosa.txt", &["".to_string()]));
        assert!(!es_ignorada("/home/pato/cosa.txt", &["   ".to_string()]));
    }

    #[test]
    fn las_raices_son_la_casa_y_lo_que_este_montado() {
        let montajes = "\
/dev/nvme0n1p2 / ext4 rw,relatime 0 0
proc /proc proc rw,nosuid 0 0
/dev/sdb1 /run/media/pato/USB vfat rw,nosuid 0 0
/dev/sdc1 /mnt/respaldo ext4 rw 0 0
";
        assert_eq!(
            raices_desde(Some(PathBuf::from("/home/pato")), montajes),
            vec![
                PathBuf::from("/home/pato"),
                PathBuf::from("/run/media/pato/USB"),
                PathBuf::from("/mnt/respaldo"),
            ]
        );
    }

    #[test]
    fn la_raiz_del_sistema_no_es_una_raiz_del_escaneo() {
        // `/` está montado siempre y recorrerlo sería indexar `/usr` y `/var`
        // enteros: miles de archivos del sistema que nadie busca por nombre
        // desde un lanzador, y que cambian con cada actualización.
        let montajes = "/dev/nvme0n1p2 / ext4 rw 0 0\n";
        assert_eq!(
            raices_desde(Some(PathBuf::from("/home/pato")), montajes),
            vec![PathBuf::from("/home/pato")]
        );
    }

    #[test]
    fn un_punto_de_montaje_con_espacios_se_entiende() {
        // `/proc/mounts` escapa el espacio como `\040`. Sin deshacerlo, el
        // disco «Mi Disco» se buscaría en una ruta que no existe y no se
        // indexaría nada de él, sin ningún error.
        let montajes = r"/dev/sdb1 /run/media/pato/Mi\040Disco vfat rw 0 0".to_string() + "\n";
        assert_eq!(
            raices_desde(None, &montajes),
            vec![PathBuf::from("/run/media/pato/Mi Disco")]
        );
    }

    #[test]
    fn una_casa_relativa_no_es_una_raiz() {
        // `dirs` mira que `HOME` no esté vacío, no que sea absoluto. Un
        // recorrido desde una ruta relativa arrancaría en el directorio de
        // trabajo del daemon, que lo elige systemd.
        assert!(raices_desde(Some(PathBuf::from("casa")), "").is_empty());
        assert!(raices_desde(Some(PathBuf::from("")), "").is_empty());
        assert!(raices_desde(None, "").is_empty());
    }

    #[test]
    fn una_regla_que_contiene_a_la_raiz_no_la_poda() {
        // Las dos funciones estaban bien por separado y mal juntas:
        // `raices_desde` devuelve `/run/media/pato/USB` y `/run` está en la
        // lista de rutas del sistema, así que el recorrido se podaba en la raíz
        // misma. En Arch y en Fedora eso es todo pendrive que se enchufe, sin
        // ningún aviso.
        let lista = ignoradas();
        let raiz = PathBuf::from("/run/media/pato/USB");

        assert!(
            es_ignorada("/run/media/pato/USB", &lista),
            "la regla suelta sí la saca, que es de donde venía el problema"
        );

        let reglas = reglas_para(&raiz, &lista);
        assert!(!es_ignorada("/run/media/pato/USB", &reglas));
        assert!(!es_ignorada(
            "/run/media/pato/USB/Documentos/carta.odt",
            &reglas
        ));

        assert!(
            es_ignorada("/run/media/pato/USB/proyecto/node_modules", &reglas),
            "las reglas por nombre siguen valiendo adentro de la raíz"
        );
    }

    #[test]
    fn recorrer_la_casa_no_deja_de_excluir_el_sistema() {
        // Lo de arriba no puede aflojar la lista para la raíz normal: lo que no
        // contiene a la casa se sigue aplicando entero.
        let reglas = reglas_para(Path::new("/home/pato"), &ignoradas());

        assert!(es_ignorada("/proc/1/status", &reglas));
        assert!(es_ignorada("/run/user/1000/socket", &reglas));
        assert!(es_ignorada("/home/pato/proyecto/target/x", &reglas));
        assert!(!es_ignorada("/home/pato/proyectos/dev/main.rs", &reglas));
    }

    #[test]
    fn sin_nada_escrito_conviene_escanear() {
        assert!(conviene_escanear(None, 1_000_000, VIGENCIA_MS));
    }

    fn completo_en(cuando: u64) -> Estado {
        Estado {
            last_scan_time: Some(cuando),
            indexed_item_count: 100,
            schema_version: contrato::VERSION,
            scan_state: Some(contrato::ESTADO_COMPLETO.to_string()),
            scan_state_time: Some(cuando),
            scan_state_ttl_ms: Some(VENCE_EN_CURSO_MS),
        }
    }

    #[test]
    fn un_indice_fresco_no_se_rehace() {
        let estado = completo_en(1_000_000);
        assert!(!conviene_escanear(
            Some(&estado),
            1_000_000 + VIGENCIA_MS - 1,
            VIGENCIA_MS
        ));
    }

    #[test]
    fn pasada_la_vigencia_se_rehace() {
        let estado = completo_en(1_000_000);
        assert!(conviene_escanear(
            Some(&estado),
            1_000_000 + VIGENCIA_MS,
            VIGENCIA_MS
        ));
    }

    #[test]
    fn mientras_otro_escanea_no_se_arranca_un_segundo() {
        // Puede ser el gestor de archivos sin actualizar. Tantivy no dejaría
        // entrar al segundo igual, pero enterarse antes evita recorrer el disco
        // entero para chocar al final.
        let mut estado = completo_en(0);
        estado.scan_state = Some(contrato::ESTADO_EN_CURSO.to_string());
        estado.scan_state_time = Some(1_000_000);
        estado.scan_state_ttl_ms = Some(VENCE_EN_CURSO_MS);

        assert!(!conviene_escanear(Some(&estado), 1_000_001, VIGENCIA_MS));
    }

    #[test]
    fn un_en_curso_vencido_no_traba_para_siempre() {
        // Un escaneo que murió de golpe —la sesión se cerró en el medio— deja
        // el estado en «en curso» y nadie lo corrige. Sin el vencimiento, el
        // índice no se rehace nunca más.
        let mut estado = completo_en(0);
        estado.scan_state = Some(contrato::ESTADO_EN_CURSO.to_string());
        estado.scan_state_time = Some(1_000_000);
        estado.scan_state_ttl_ms = Some(VENCE_EN_CURSO_MS);

        assert!(conviene_escanear(
            Some(&estado),
            1_000_000 + VENCE_EN_CURSO_MS,
            VIGENCIA_MS
        ));
    }

    #[test]
    fn una_fecha_del_futuro_manda_rehacer() {
        // Reloj corregido hacia atrás. Restar daría la vuelta en `u64` y el
        // índice quedaría «fresco» por varios millones de años.
        let estado = completo_en(2_000_000);
        assert!(conviene_escanear(Some(&estado), 1_000_000, VIGENCIA_MS));
    }

    #[test]
    fn un_esquema_de_otra_version_se_rehace_aunque_este_fresco() {
        let mut estado = completo_en(1_000_000);
        estado.schema_version = contrato::VERSION + 1;
        assert!(conviene_escanear(Some(&estado), 1_000_001, VIGENCIA_MS));
    }

    #[test]
    fn el_estado_va_y_vuelve_con_los_nombres_del_contrato() {
        // Los nombres de los campos son lo que el gestor lee de su lado, así
        // que se comprueban contra el JSON y no contra la estructura: renombrar
        // uno en Rust no rompe nada acá si sólo se mira `Estado`.
        let base = directorio("estado");
        escribir_estado(&base, contrato::ESTADO_COMPLETO, Some(42), 1_000_000);

        let texto = std::fs::read_to_string(contrato::archivo_de_estado(&base)).unwrap();
        let crudo: serde_json::Value = serde_json::from_str(&texto).unwrap();

        assert_eq!(crudo["last_scan_time"], 1_000_000u64);
        assert_eq!(crudo["indexed_item_count"], 42u64);
        assert_eq!(crudo["schema_version"], contrato::VERSION);
        assert_eq!(crudo["scan_state"], "complete");
        assert_eq!(crudo["scan_state_time"], 1_000_000u64);
        assert_eq!(crudo["scan_state_ttl_ms"], VENCE_EN_CURSO_MS);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn un_escaneo_fallido_no_borra_lo_que_se_sabia() {
        // Si un fallo pisara la fecha y el conteo, el gestor pasaría de «77 mil
        // archivos, de hace una hora» a no saber nada. Un fallo puede dejar el
        // índice viejo; no puede además borrar lo que se sabía de él.
        let base = directorio("fallo");
        escribir_estado(&base, contrato::ESTADO_COMPLETO, Some(77), 1_000_000);
        escribir_estado(&base, contrato::ESTADO_FALLADO, None, 2_000_000);

        let estado = leer_estado(&base).expect("tiene que seguir habiendo estado");
        assert_eq!(estado.scan_state.as_deref(), Some(contrato::ESTADO_FALLADO));
        assert_eq!(
            estado.last_scan_time,
            Some(1_000_000),
            "la fecha del último escaneo bueno se conserva"
        );
        assert_eq!(estado.indexed_item_count, 77, "y el conteo también");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn un_estado_de_una_version_sin_los_campos_nuevos_se_lee_entero() {
        // Lo escribió un gestor viejo. Si el parseo fallara se perderían
        // también la versión y la fecha, que sí están: agregar un campo no
        // puede costar más que no tenerlo.
        let base = directorio("estado-viejo");
        let ruta = contrato::archivo_de_estado(&base);
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(
            &ruta,
            r#"{"last_scan_time":123,"indexed_item_count":9,"schema_version":1}"#,
        )
        .unwrap();

        let estado = leer_estado(&base).expect("tiene que leerse");
        assert_eq!(estado.last_scan_time, Some(123));
        assert_eq!(estado.indexed_item_count, 9);
        assert_eq!(estado.scan_state, None);

        let _ = std::fs::remove_dir_all(&base);
    }

    /// Un arbolito de mentira para escanear.
    fn arbol(base: &Path) {
        let disco = base.join("disco");
        std::fs::create_dir_all(disco.join("Documentos")).unwrap();
        std::fs::create_dir_all(disco.join("proyecto/target")).unwrap();
        std::fs::write(disco.join("Documentos/informe anual.pdf"), b"x").unwrap();
        std::fs::write(disco.join("Documentos/notas.md"), b"xx").unwrap();
        std::fs::write(disco.join("proyecto/main.rs"), b"xxx").unwrap();
        std::fs::write(disco.join("proyecto/target/basura.o"), b"xxxx").unwrap();
    }

    #[test]
    fn lo_que_se_escanea_se_puede_buscar() {
        // La prueba de punta a punta, y la que de verdad importa: lo que este
        // módulo **escribe** tiene que poder abrirlo y leerlo el proveedor, que
        // es el mismo código con el que el gestor de archivos lo va a leer.
        let base = directorio("punta-a-punta");
        arbol(&base);

        let cuantas = escanear(
            &base,
            &[base.join("disco")],
            &solo_target(),
            &AtomicBool::new(false),
            1_000_000,
        )
        .expect("tiene que poder escanear");

        // Cinco: los dos directorios, los dos archivos de Documentos y el
        // `main.rs`. Lo de `target/` no, y el propio `target/` tampoco.
        assert_eq!(cuantas, 5, "lo ignorado no se cuenta");

        let indice = super::super::Indice::abrir(&contrato::directorio_del_indice(&base))
            .expect("el índice recién escrito tiene que abrirse");

        let encontrados = indice.buscar("notas", 10);
        assert_eq!(encontrados.len(), 1);
        assert!(
            encontrados[0].id.ends_with("notas.md"),
            "{:?}",
            encontrados[0].id
        );

        assert_eq!(
            indice.buscar("basura", 10).len(),
            0,
            "lo que está en target/ no se indexó"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn el_estado_queda_completo_y_con_el_conteo() {
        let base = directorio("estado-tras-escanear");
        arbol(&base);

        escanear(
            &base,
            &[base.join("disco")],
            &solo_target(),
            &AtomicBool::new(false),
            1_000_000,
        )
        .unwrap();

        let estado = leer_estado(&base).expect("tiene que haber estado");
        assert_eq!(
            estado.scan_state.as_deref(),
            Some(contrato::ESTADO_COMPLETO)
        );
        assert_eq!(estado.indexed_item_count, 5);
        assert_eq!(estado.last_scan_time, Some(1_000_000));
        assert_eq!(estado.schema_version, contrato::VERSION);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn rehacer_el_indice_no_acumula_ni_deja_lo_borrado() {
        // El escaneo vacía y vuelve a llenar. Sin el vaciado, cada pasada
        // duplicaría todo; sin rehacer, un archivo borrado seguiría
        // apareciendo y al abrirlo no habría nada.
        let base = directorio("rehacer");
        arbol(&base);
        let disco = base.join("disco");

        escanear(
            &base,
            std::slice::from_ref(&disco),
            &solo_target(),
            &AtomicBool::new(false),
            1,
        )
        .unwrap();

        std::fs::remove_file(disco.join("Documentos/notas.md")).unwrap();
        let cuantas = escanear(
            &base,
            std::slice::from_ref(&disco),
            &solo_target(),
            &AtomicBool::new(false),
            2,
        )
        .unwrap();

        assert_eq!(cuantas, 4, "uno menos que antes");

        let indice = super::super::Indice::abrir(&contrato::directorio_del_indice(&base)).unwrap();
        assert_eq!(
            indice.buscar("notas", 10).len(),
            0,
            "lo borrado del disco no puede seguir en el índice"
        );
        assert_eq!(
            indice.buscar("informe", 10).len(),
            1,
            "y lo que sigue estando no se duplicó"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn cancelar_guarda_lo_que_se_alcanzo_a_recorrer() {
        // Tirar lo recorrido dejaría al lanzador sin archivos por haber
        // cancelado, que es peor que tener una parte. El estado lo dice.
        let base = directorio("cancelado");
        arbol(&base);

        let cancelar = AtomicBool::new(true);
        let cuantas = escanear(
            &base,
            &[base.join("disco")],
            &solo_target(),
            &cancelar,
            1_000_000,
        )
        .expect("cancelar no es un error");

        assert_eq!(cuantas, 0, "se canceló antes de la primera entrada");

        let estado = leer_estado(&base).expect("tiene que haber estado");
        assert_eq!(
            estado.scan_state.as_deref(),
            Some(contrato::ESTADO_CANCELADO),
            "y quien lea tiene que poder distinguirlo de un índice completo"
        );

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn un_indice_de_otro_esquema_se_rehace_en_vez_de_fallar() {
        // Vaciarlo en el lugar conservaría el esquema viejo y el nuevo no se
        // podría crear encima. Pasa al subir la versión del esquema, y tiene
        // que arreglarse solo: nadie va a borrar un directorio de la caché a
        // mano.
        let base = directorio("otro-esquema");
        arbol(&base);

        let ruta = contrato::directorio_del_indice(&base);
        std::fs::create_dir_all(&ruta).unwrap();
        let mut otro = tantivy::schema::Schema::builder();
        otro.add_text_field("otra_cosa", tantivy::schema::STRING);
        tantivy::Index::create_in_dir(&ruta, otro.build()).unwrap();

        let cuantas = escanear(
            &base,
            &[base.join("disco")],
            &solo_target(),
            &AtomicBool::new(false),
            1_000_000,
        )
        .expect("tiene que rehacerlo, no fallar");
        assert_eq!(cuantas, 5);

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn mientras_otro_tiene_el_indice_tomado_esto_no_escribe() {
        // Es la transición: el gestor de archivos sin actualizar todavía
        // escanea. Tantivy admite un solo escritor y se lo asegura con un
        // bloqueo; lo que importa es que acá eso sea un «no ahora» y no un
        // pánico ni un índice a medias.
        let base = directorio("dos-escritores");
        arbol(&base);

        let ruta = contrato::directorio_del_indice(&base);
        std::fs::create_dir_all(&ruta).unwrap();
        let (esquema, _) = contrato::esquema();
        let ajeno = tantivy::Index::create_in_dir(&ruta, esquema).unwrap();
        let _tomado = ajeno
            .writer::<tantivy::TantivyDocument>(15_000_000)
            .expect("el otro toma el escritor primero");

        let resultado = escanear(
            &base,
            &[base.join("disco")],
            &solo_target(),
            &AtomicBool::new(false),
            1_000_000,
        );

        assert!(
            resultado.is_err(),
            "con el índice tomado no se puede escribir, y eso se dice"
        );

        let _ = std::fs::remove_dir_all(&base);
    }
}
