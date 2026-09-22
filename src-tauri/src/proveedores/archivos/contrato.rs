//! El contrato del índice: dónde está, cómo se llama cada campo y cómo se dice
//! en qué quedó el último escaneo.
//!
//! # Hay una copia de esto en el otro repositorio
//!
//! `vasak-file-manager` tiene el gemelo de este archivo, y es a propósito: son
//! dos programas que no comparten código y lo único que comparten es esto. La
//! copia no es un descuido que haya que unificar algún día — mientras los dos
//! sean binarios distintos que abren el mismo directorio, el contrato existe
//! igual, y tenerlo escrito de los dos lados con una prueba que lo fija es lo
//! que hace que romperlo falle en vez de vaciar la búsqueda en silencio.
//!
//! # Por qué importa tanto para algo tan chico
//!
//! Porque romperlo **no falla**. Si acá se renombra un campo o se mueve el
//! directorio, el otro sigue compilando, sigue arrancando y simplemente deja de
//! encontrar archivos: una lista vacía, que es indistinguible de «no hay nada
//! que coincida». No hay error, no hay registro, no hay nada que mirar.
//!
//! Por eso los nombres están en constantes y no sueltos en la llamada, y por
//! eso abajo hay pruebas que los fijan: para que cambiarlos sea un acto
//! deliberado que rompe algo visible, y no un renombre que alguien hace de paso
//! en un repositorio sin mirar el otro.
//!
//! # El dueño cambió
//!
//! Hasta la 0.12 el gestor de archivos escribía y el lanzador leía. Ahora es al
//! revés: acá se escanea y se escribe, porque el lanzador es el que está
//! prendido —unidad de systemd, `Type=dbus`, con la sesión— y el gestor es una
//! ventana que se abre y se cierra. Ver Vasak-OS/vasak-prism#33.
//!
//! **La ruta no cambió y el índice no se rehace por eso.** Ya vivía en la caché
//! compartida, que no cuelga del nombre de ninguna de las dos aplicaciones, así
//! que el cambio de dueño es sólo eso: el que escribe. El índice que haya se
//! hereda tal cual.
//!
//! # Y tantivy también es parte del contrato
//!
//! Los dos programas tienen que usar la **misma versión de tantivy**: el
//! formato en disco cambia entre versiones, así que subirla de un lado solo
//! deja al otro sin poder abrir el índice. Hoy los dos están en 0.22 y la
//! última publicada es la 0.26: subir es una tanda coordinada entre los dos
//! repositorios, no una actualización de rutina.

use std::path::{Path, PathBuf};
use tantivy::schema::{
    Field, IndexRecordOption, Schema, TextFieldIndexing, TextOptions, FAST, STORED, STRING,
};

/// La ruta completa del archivo, tal cual, para poder abrirlo.
pub const CAMPO_RUTA: &str = "path";
/// El nombre, tokenizado, que es contra lo que se busca.
pub const CAMPO_NOMBRE: &str = "name";
/// El nombre en minúsculas, para comparar sin importar cómo se escribió.
pub const CAMPO_NOMBRE_MINUSCULA: &str = "name_lower";
/// Si es un archivo. Va como número porque tantivy no tiene booleanos.
pub const CAMPO_ES_ARCHIVO: &str = "is_file";
/// Si es un directorio.
pub const CAMPO_ES_DIRECTORIO: &str = "is_dir";
/// Cuándo se modificó, en milisegundos desde la época.
pub const CAMPO_MODIFICADO: &str = "modified_time";
/// El tamaño en bytes.
pub const CAMPO_TAMANIO: &str = "size";

/// El directorio compartido de VasakOS dentro de la caché del usuario.
///
/// Compartido a propósito: no cuelga del nombre de ninguna de las dos
/// aplicaciones porque no es de ninguna de las dos. Que ya estuviera acá es lo
/// que permite que el cambio de dueño no mueva nada.
pub const COMPARTIDO: &str = "vasak";
/// El subdirectorio donde vive todo lo de la búsqueda global.
pub const DIRECTORIO: &str = "global-search";
/// El índice propiamente dicho, adentro del directorio de la versión.
pub const INDICE: &str = "index";
/// Lo que se sabe del último escaneo. **Fuera** del directorio de la versión.
pub const ESTADO: &str = "status.json";

/// Hay un escaneo corriendo. El índice está a medio construir y va a cambiar.
/// Es el único estado no terminal, y el único que vence.
pub const ESTADO_EN_CURSO: &str = "in_progress";
/// Terminó y recorrió todo. Acá —y sólo acá— la ausencia es ausencia.
pub const ESTADO_COMPLETO: &str = "complete";
/// Alguien lo paró antes de terminar. Lo indexado sirve, pero falta.
pub const ESTADO_CANCELADO: &str = "cancelled";
/// Se cortó por un error. Igual que el anterior para quien lee.
pub const ESTADO_FALLADO: &str = "failed";

/// La versión del esquema, que va en la ruta.
///
/// Subirla cambia el directorio, así que el índice viejo deja de usarse solo y
/// no hay que acordarse de borrar nada: un esquema nuevo nunca se lee con el
/// código viejo, que es la forma de fallar que no se nota.
pub const VERSION: u32 = 1;

/// Si un `EN_CURSO` todavía vale, o quedó de un escaneo que murió de golpe.
///
/// El acuerdo entre los dos programas es esta frase y ninguna constante: **si
/// la fecha no está entre ahora y ahora más el vencimiento declarado, esto no
/// está vivo**. El vencimiento lo declara el que escribe, porque el que sabe
/// cuánto puede tardar razonablemente un escaneo es el que escanea.
///
/// Una fecha en el **futuro** cuenta como vencida. Un reloj corregido hacia
/// atrás, o una máquina que arrancó con la hora mal, dejarían un `EN_CURSO` que
/// no vence nunca y un «indexando» eterno que nadie puede destrabar.
pub fn en_curso_sigue_vivo(escrito_en: u64, vence_en_ms: u64, ahora: u64) -> bool {
    escrito_en <= ahora && ahora < escrito_en.saturating_add(vence_en_ms)
}

/// Los campos del esquema, ya resueltos contra un índice abierto.
///
/// Tantivy identifica los campos por un número que asigna al construir el
/// esquema, no por su nombre, así que hay que quedarse con ellos.
#[derive(Debug, Clone)]
pub struct Campos {
    pub ruta: Field,
    pub nombre: Field,
    pub nombre_minuscula: Field,
    pub es_archivo: Field,
    pub es_directorio: Field,
    pub modificado: Field,
    pub tamanio: Field,
}

/// El esquema del índice, y sus campos.
///
/// Tiene que dar exactamente lo mismo de los dos lados: un campo declarado
/// `STRING` de un lado y `TEXT` del otro no es el mismo campo, aunque se llame
/// igual, y la consulta no encuentra nada.
///
/// Acá hace falta entero —y no sólo los tres campos que el proveedor lee—
/// porque desde la 0.12 este lado **crea** el índice. Un esquema incompleto no
/// daría un error: daría un índice que el gestor de archivos no puede usar.
pub fn esquema() -> (Schema, Campos) {
    let mut constructor = Schema::builder();

    // El nombre va con frecuencias y posiciones porque es contra lo que se
    // busca de verdad, con tolerancia a errores de tipeo.
    let indexado_del_nombre = TextFieldIndexing::default()
        .set_tokenizer("default")
        .set_index_option(IndexRecordOption::WithFreqsAndPositions);
    let opciones_del_nombre = TextOptions::default()
        .set_indexing_options(indexado_del_nombre)
        .set_stored();

    let ruta = constructor.add_text_field(CAMPO_RUTA, STRING | STORED);
    let nombre = constructor.add_text_field(CAMPO_NOMBRE, opciones_del_nombre);
    let nombre_minuscula = constructor.add_text_field(CAMPO_NOMBRE_MINUSCULA, STRING | STORED);

    let es_archivo = constructor.add_u64_field(CAMPO_ES_ARCHIVO, FAST | STORED);
    let es_directorio = constructor.add_u64_field(CAMPO_ES_DIRECTORIO, FAST | STORED);
    let modificado = constructor.add_u64_field(CAMPO_MODIFICADO, FAST | STORED);
    let tamanio = constructor.add_u64_field(CAMPO_TAMANIO, FAST | STORED);

    let esquema = constructor.build();
    (
        esquema,
        Campos {
            ruta,
            nombre,
            nombre_minuscula,
            es_archivo,
            es_directorio,
            modificado,
            tamanio,
        },
    )
}

/// El directorio compartido en la caché del usuario.
///
/// En caché y no en datos porque el índice es contenido derivado del disco: se
/// rehace entero escaneando, no hay nada que no se pueda recuperar, y no tiene
/// por qué sobrevivir a un borrado ni entrar en una copia de respaldo.
pub fn base_de_cache() -> Option<PathBuf> {
    base_de_cache_bajo(dirs::cache_dir())
}

/// Lo mismo, recibiendo la base en vez de leerla del entorno.
///
/// Aparte porque el entorno es global al proceso y las pruebas corren en
/// paralelo: una que escriba una variable decide al azar el resultado de otra.
///
/// La base sale de `dirs`, que ya trae la regla de que una ruta XDG relativa se
/// ignora. Lo que `dirs` **no** hace es mirar `HOME`, del que sólo comprueba
/// que no esté vacío: si `HOME` es relativo devuelve una base relativa, y el
/// índice terminaría colgando del directorio de trabajo de quien haya lanzado
/// el programa, que en un servicio de systemd puede ser cualquiera. El filtro
/// de acá cierra esa mitad, y es la misma línea que `crate::rutas::base`.
pub fn base_de_cache_bajo(base: Option<PathBuf>) -> Option<PathBuf> {
    Some(crate::rutas::base(base)?.join(COMPARTIDO))
}

/// Dónde vive el índice, a partir del directorio compartido.
pub fn directorio_del_indice(base: &Path) -> PathBuf {
    base.join(DIRECTORIO)
        .join(format!("v{VERSION}"))
        .join(INDICE)
}

/// Dónde vive lo que se sabe del último escaneo.
///
/// Queda **afuera** del directorio de la versión, y es deliberado. Si cayera
/// adentro, al subir a `v2` un lector viejo no encontraría ni el índice ni el
/// archivo que le explicaría por qué: «existe y es de otra versión» le llegaría
/// como «no existe». Sería poner el cartel del otro lado de la puerta cerrada.
pub fn archivo_de_estado(base: &Path) -> PathBuf {
    base.join(DIRECTORIO).join(ESTADO)
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use tantivy::schema::FieldType;

    #[test]
    fn los_nombres_de_los_campos_no_se_tocan() {
        // Son la mitad del contrato con `vasak-file-manager`, que los tiene
        // escritos a mano de su lado. Renombrar uno acá lo deja sin encontrar
        // archivos, y sin ningún error: una lista vacía.
        //
        // Si esta prueba molesta porque el esquema **tiene** que cambiar, el
        // cambio es deliberado y va con el del otro repositorio en la misma
        // tanda. De eso se trata.
        let (esquema, _) = esquema();
        let nombres: Vec<&str> = esquema.fields().map(|(_, e)| e.name()).collect();

        assert_eq!(
            nombres,
            vec![
                "path",
                "name",
                "name_lower",
                "is_file",
                "is_dir",
                "modified_time",
                "size"
            ]
        );
    }

    #[test]
    fn el_orden_de_los_campos_tampoco() {
        // No es cosmético: tantivy identifica cada campo por el número que le
        // asigna al construir el esquema, en orden de declaración. Reordenar
        // dos campos del mismo tipo deja los nombres iguales y **cambia qué
        // campo es cuál** para un índice ya escrito.
        let (_, campos) = esquema();
        assert_eq!(campos.ruta.field_id(), 0);
        assert_eq!(campos.nombre.field_id(), 1);
        assert_eq!(campos.nombre_minuscula.field_id(), 2);
        assert_eq!(campos.es_archivo.field_id(), 3);
        assert_eq!(campos.es_directorio.field_id(), 4);
        assert_eq!(campos.modificado.field_id(), 5);
        assert_eq!(campos.tamanio.field_id(), 6);
    }

    #[test]
    fn la_ruta_del_indice_tampoco() {
        // La otra mitad. El gestor la arma con esta misma forma, y con el mismo
        // directorio compartido, que no es de ninguna de las dos apps.
        let base = Path::new("/casa/.cache/vasak");

        assert_eq!(
            directorio_del_indice(base),
            Path::new("/casa/.cache/vasak/global-search/v1/index")
        );
    }

    #[test]
    fn el_estado_queda_fuera_del_directorio_de_la_version() {
        let base = Path::new("/casa/.cache/vasak");

        assert_eq!(
            archivo_de_estado(base),
            Path::new("/casa/.cache/vasak/global-search/status.json")
        );

        // Y la forma, no sólo la cadena: el estado no puede quedar por debajo
        // del directorio que se renueva al subir la versión.
        let estado = archivo_de_estado(base);
        let indice = directorio_del_indice(base);
        let directorio_de_la_version = indice.parent().unwrap();

        assert!(
            !estado.starts_with(directorio_de_la_version),
            "subir la versión se llevaría el estado puesto"
        );
    }

    #[test]
    fn la_base_cuelga_del_directorio_compartido() {
        assert_eq!(
            base_de_cache_bajo(Some(PathBuf::from("/casa/.cache"))),
            Some(PathBuf::from("/casa/.cache/vasak"))
        );
    }

    #[test]
    fn una_base_que_no_es_absoluta_no_sirve() {
        // La mitad que `dirs` no cubre: de `HOME` sólo comprueba que no esté
        // vacío, así que un `HOME` relativo le sale como base relativa. Y una
        // ruta que no arranca en la raíz se resuelve contra el directorio de
        // trabajo de quien haya lanzado el programa, que en un servicio de
        // systemd puede ser cualquiera: ahora que este lado **escribe**, el
        // índice quedaría escrito en un lugar impredecible.
        for base in ["", "cache", "./cache", "../cache"] {
            assert_eq!(
                base_de_cache_bajo(Some(PathBuf::from(base))),
                None,
                "«{base}» no es una ruta absoluta"
            );
        }
    }

    #[test]
    fn sin_base_no_se_inventa_una() {
        assert_eq!(base_de_cache_bajo(None), None);
    }

    #[test]
    fn los_estados_del_escaneo_tampoco_se_tocan() {
        // El gestor los compara contra cadenas escritas a mano de su lado.
        assert_eq!(ESTADO_EN_CURSO, "in_progress");
        assert_eq!(ESTADO_COMPLETO, "complete");
        assert_eq!(ESTADO_CANCELADO, "cancelled");
        assert_eq!(ESTADO_FALLADO, "failed");
    }

    #[test]
    fn un_en_curso_vence_por_su_propio_vencimiento() {
        let escrito = 1_000_000u64;
        let vence_en = 60_000u64;

        assert!(en_curso_sigue_vivo(escrito, vence_en, escrito));
        assert!(en_curso_sigue_vivo(
            escrito,
            vence_en,
            escrito + vence_en - 1
        ));
        assert!(!en_curso_sigue_vivo(escrito, vence_en, escrito + vence_en));
    }

    #[test]
    fn una_fecha_del_futuro_cuenta_como_vencida() {
        // Un reloj corregido hacia atrás deja un «en curso» que nunca vence,
        // porque `escrito + vencimiento` siempre es mayor que ahora. Sin esto
        // el estado queda en «indexando» para siempre.
        let ahora = 1_000_000u64;
        assert!(!en_curso_sigue_vivo(ahora + 1, 60_000, ahora));
    }

    #[test]
    fn un_vencimiento_enorme_no_da_la_vuelta() {
        // `escrito + vencimiento` se puede pasar de `u64` y envolver a un
        // número chico, y entonces un «en curso» recién escrito se leería como
        // vencido. Con saturación se queda arriba de todo.
        assert!(en_curso_sigue_vivo(1_000, u64::MAX, 2_000));
    }

    #[test]
    fn el_nombre_se_indexa_distinto_de_la_ruta() {
        // No alcanza con que los nombres coincidan: dos campos que se llamen
        // igual pero estén indexados distinto no son el mismo campo, y la
        // consulta no encuentra nada.
        let (esquema, campos) = esquema();

        assert_eq!(
            esquema
                .get_field_entry(campos.nombre)
                .field_type()
                .get_index_record_option(),
            Some(IndexRecordOption::WithFreqsAndPositions),
            "el nombre es contra lo que se busca"
        );
        assert_eq!(
            esquema
                .get_field_entry(campos.ruta)
                .field_type()
                .get_index_record_option(),
            Some(IndexRecordOption::Basic),
            "la ruta va entera"
        );
    }

    /// Con qué tokenizador se indexa un campo de texto.
    fn tokenizador(esquema: &Schema, campo: Field) -> Option<&str> {
        match esquema.get_field_entry(campo).field_type() {
            FieldType::Str(opciones) => opciones.get_indexing_options().map(|o| o.tokenizer()),
            _ => None,
        }
    }

    #[test]
    fn el_tokenizador_de_cada_campo_tampoco_se_toca() {
        // El detalle de indexado y el tokenizador son dos cosas separadas, y la
        // prueba de arriba sólo fija la primera. Quien cambie el tokenizador de
        // `name` de `default` a `raw` deja aquella prueba en verde.
        //
        // Importa porque el proveedor no consulta con `QueryParser`: arma el
        // término a mano, en minúsculas, y lo mete en una consulta difusa. Que
        // los términos guardados estén en minúsculas no lo da el detalle de
        // indexado, lo da el tokenizador `default`, que es el que baja.
        let (esquema, campos) = esquema();

        assert_eq!(tokenizador(&esquema, campos.nombre), Some("default"));
        assert_eq!(tokenizador(&esquema, campos.ruta), Some("raw"));
        assert_eq!(tokenizador(&esquema, campos.nombre_minuscula), Some("raw"));
    }
}
