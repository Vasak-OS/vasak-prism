//! Leer un archivo `.desktop` como lo lee el resto del escritorio.
//!
//! El parser que había en `vasak-desktop` miraba cuatro claves —`Name`, `Exec`,
//! `Icon`, `Comment`— y nada más. Eso dejaba dos agujeros que se veían todos los
//! días: aparecían en los resultados entradas que ningún menú debería mostrar
//! (las de otros escritorios, las auxiliares, las de programas desinstalados), y
//! no se podía encontrar una aplicación por su nombre en español ni por sus
//! palabras clave.
//!
//! Acá no hay entrada y salida: se le pasa el texto del archivo y devuelve lo
//! que dice. Lo único que toca el disco es `puede_ejecutarse`, y está aparte
//! justamente para que todo lo demás se pueda probar.

/// Una acción de la entrada: «Ventana privada», «Componer un correo».
///
/// Son resultados por derecho propio. Un lanzador que sólo ofrece «Firefox»
/// obliga a abrir el navegador y después buscar el menú; uno que ofrece
/// «Firefox — Ventana privada» te deja en el lugar.
#[derive(Debug, Clone, PartialEq)]
pub struct Accion {
    pub id: String,
    pub nombre: String,
    pub exec: String,
    pub icono: Option<String>,
}

/// Una entrada del escritorio que se puede mostrar y lanzar.
#[derive(Debug, Clone, PartialEq)]
pub struct Entrada {
    /// El nombre del archivo, que es lo que identifica a la entrada.
    pub id: String,
    pub nombre: String,
    pub nombre_generico: Option<String>,
    pub comentario: Option<String>,
    pub palabras_clave: Vec<String>,
    pub icono: Option<String>,
    /// La línea `Exec` tal cual: el desarmado se hace al lanzar, con el módulo
    /// `exec`, porque necesita datos que sólo tiene quien lanza.
    pub exec: String,
    /// El programa de `TryExec`, si la entrada declara uno.
    pub try_exec: Option<String>,
    /// La entrada necesita una terminal. Sin mirar esto se lanzan sin ella y no
    /// se ve nada: el proceso arranca, escribe en una salida que no existe y
    /// termina.
    pub terminal: bool,
    pub acciones: Vec<Accion>,
}

impl Entrada {
    /// Si el programa de `TryExec` está en el `PATH`.
    ///
    /// Lo único de este módulo que mira el disco. Una entrada cuyo `TryExec` no
    /// existe es de un programa desinstalado que dejó su `.desktop`: aparece en
    /// la lista, se elige, y no abre nada.
    pub fn puede_ejecutarse(&self) -> bool {
        let Some(programa) = &self.try_exec else {
            return true;
        };
        esta_en_el_path(programa)
    }
}

fn esta_en_el_path(programa: &str) -> bool {
    if programa.contains('/') {
        return std::path::Path::new(programa).exists();
    }

    std::env::var_os("PATH")
        .map(|rutas| {
            std::env::split_paths(&rutas).any(|directorio| directorio.join(programa).exists())
        })
        .unwrap_or(false)
}

/// Lee una entrada del escritorio.
///
/// `idioma` es el código de la sesión (`es`, `es_AR`); `escritorios` es
/// `XDG_CURRENT_DESKTOP` ya partido. Devuelve `None` cuando la entrada no es
/// para mostrar, con todos los motivos que el estándar define — que son varios
/// más que «no tiene nombre».
pub fn parsear(contenido: &str, id: &str, idioma: &str, escritorios: &[String]) -> Option<Entrada> {
    let grupos = agrupar(contenido);
    let principal = grupos
        .iter()
        .find(|(nombre, _)| nombre == "Desktop Entry")?;
    let claves = &principal.1;

    // Sólo las aplicaciones se lanzan. Un `Link` o un `Directory` en un lanzador
    // es una entrada que al elegirla no hace nada.
    if valor(claves, "Type")? != "Application" {
        return None;
    }

    // Las dos formas de decir «esto no se muestra». `NoDisplay` es para las
    // entradas auxiliares —la mitad de las de un escritorio lo son— y `Hidden`
    // significa que el usuario la borró: el estándar dice que hay que tratarla
    // como si el archivo no existiera.
    if bandera(claves, "NoDisplay") || bandera(claves, "Hidden") {
        return None;
    }

    if !se_muestra_en(claves, escritorios) {
        return None;
    }

    let nombre = traducido(claves, "Name", idioma)?.to_string();
    let exec = valor(claves, "Exec")?.to_string();

    Some(Entrada {
        id: id.to_string(),
        nombre,
        nombre_generico: traducido(claves, "GenericName", idioma).map(str::to_string),
        comentario: traducido(claves, "Comment", idioma).map(str::to_string),
        palabras_clave: lista(traducido(claves, "Keywords", idioma)),
        icono: valor(claves, "Icon").map(str::to_string),
        exec,
        try_exec: valor(claves, "TryExec").map(str::to_string),
        terminal: bandera(claves, "Terminal"),
        acciones: acciones(claves, &grupos, idioma),
    })
}

/// Los grupos del archivo, en orden, con sus pares clave/valor.
///
/// En orden y no en un mapa porque las acciones se nombran por su grupo y hay
/// que poder encontrarlas; y por grupo y no todo junto porque una clave `Name`
/// de una acción no es el nombre de la entrada — leerlo todo plano es cómo el
/// parser viejo podía terminar con el nombre de la última acción.
fn agrupar(contenido: &str) -> Vec<(String, Vec<(String, String)>)> {
    let mut grupos: Vec<(String, Vec<(String, String)>)> = Vec::new();

    for linea in contenido.lines() {
        let linea = linea.trim();

        if linea.is_empty() || linea.starts_with('#') {
            continue;
        }

        if let Some(nombre) = linea.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            grupos.push((nombre.to_string(), Vec::new()));
            continue;
        }

        let Some((clave, valor)) = linea.split_once('=') else {
            continue;
        };

        if let Some((_, claves)) = grupos.last_mut() {
            claves.push((clave.trim().to_string(), valor.trim().to_string()));
        }
    }

    grupos
}

fn valor<'a>(claves: &'a [(String, String)], clave: &str) -> Option<&'a str> {
    claves
        .iter()
        .find(|(nombre, _)| nombre == clave)
        .map(|(_, valor)| valor.as_str())
        .filter(|valor| !valor.is_empty())
}

/// Una clave booleana. Sólo `true` es verdadero, como pide el estándar.
fn bandera(claves: &[(String, String)], clave: &str) -> bool {
    valor(claves, clave) == Some("true")
}

/// El valor en el idioma de la sesión, con la reserva del estándar.
///
/// Se prueba `clave[es_AR]` y después `clave[es]`, y recién al final la clave
/// sin idioma. Sin esto no se encuentra «Tienda» escribiendo «tienda», que es
/// como la ve el usuario en todo el resto del escritorio.
fn traducido<'a>(claves: &'a [(String, String)], clave: &str, idioma: &str) -> Option<&'a str> {
    let corto = idioma.split(['_', '.', '@']).next().unwrap_or(idioma);

    valor(claves, &format!("{clave}[{idioma}]"))
        .or_else(|| valor(claves, &format!("{clave}[{corto}]")))
        .or_else(|| valor(claves, clave))
}

/// Una lista separada por punto y coma, sin el hueco que deja el último.
fn lista(valor: Option<&str>) -> Vec<String> {
    valor
        .unwrap_or_default()
        .split(';')
        .map(str::trim)
        .filter(|parte| !parte.is_empty())
        .map(str::to_string)
        .collect()
}

/// Si la entrada corresponde a este escritorio.
///
/// La comparación es sin distinguir mayúsculas a propósito: nuestra propia
/// sesión escribe el nombre de dos formas —`Vasak` en `vasak-session` y
/// `VasakOS` en el arranque del saludador—, y una entrada que pide una y se lee
/// con la otra desaparece sin ningún síntoma. El estándar la quiere exacta;
/// aflojarla sólo puede hacer que aparezca algo que debía aparecer.
fn se_muestra_en(claves: &[(String, String)], escritorios: &[String]) -> bool {
    let coincide = |lista_de_claves: Vec<String>| {
        lista_de_claves.iter().any(|pedido| {
            escritorios
                .iter()
                .any(|actual| actual.eq_ignore_ascii_case(pedido))
        })
    };

    if let Some(solo) = valor(claves, "OnlyShowIn") {
        return coincide(lista(Some(solo)));
    }

    if let Some(nunca) = valor(claves, "NotShowIn") {
        return !coincide(lista(Some(nunca)));
    }

    true
}

/// Las acciones que la entrada declara **y** define.
///
/// Se recorre `Actions` y no los grupos del archivo: un grupo
/// `[Desktop Action x]` que no está en la lista no es una acción, es basura que
/// quedó, y el estándar dice que se ignora.
fn acciones(
    claves: &[(String, String)],
    grupos: &[(String, Vec<(String, String)>)],
    idioma: &str,
) -> Vec<Accion> {
    lista(valor(claves, "Actions"))
        .into_iter()
        .filter_map(|id| {
            let grupo = format!("Desktop Action {id}");
            let (_, claves) = grupos.iter().find(|(nombre, _)| *nombre == grupo)?;

            Some(Accion {
                nombre: traducido(claves, "Name", idioma)?.to_string(),
                exec: valor(claves, "Exec")?.to_string(),
                icono: valor(claves, "Icon").map(str::to_string),
                id,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMPLETA: &str = r#"
[Desktop Entry]
Type=Application
Name=Web Browser
Name[es]=Navegador web
GenericName=Browser
GenericName[es]=Navegador
Comment=Browse the web
Comment[es]=Navegá la web
Keywords=web;internet;
Keywords[es]=web;internet;navegador;
Icon=firefox
Exec=firefox %u
TryExec=firefox
Terminal=false
Actions=ventana-privada;perfil;

[Desktop Action ventana-privada]
Name=New private window
Name[es]=Nueva ventana privada
Exec=firefox --private-window

[Desktop Action perfil]
Name=Profile manager
Exec=firefox --ProfileManager
Icon=firefox-perfil
"#;

    fn leer(contenido: &str) -> Option<Entrada> {
        parsear(contenido, "x.desktop", "es_AR", &["Vasak".to_string()])
    }

    #[test]
    fn una_entrada_completa_se_lee_entera() {
        let entrada = leer(COMPLETA).expect("tenía que leerse");

        assert_eq!(entrada.nombre, "Navegador web");
        assert_eq!(entrada.nombre_generico.as_deref(), Some("Navegador"));
        assert_eq!(entrada.comentario.as_deref(), Some("Navegá la web"));
        assert_eq!(entrada.palabras_clave, ["web", "internet", "navegador"]);
        assert_eq!(entrada.icono.as_deref(), Some("firefox"));
        assert_eq!(entrada.exec, "firefox %u");
        assert!(!entrada.terminal);
    }

    #[test]
    fn el_idioma_cae_del_pais_al_idioma_y_del_idioma_al_original() {
        // `es_AR` no existe en el archivo, `es` sí: tiene que usar `es`.
        let entrada = leer(COMPLETA).unwrap();
        assert_eq!(entrada.nombre, "Navegador web");

        // Y un idioma sin traducir se queda con la clave sin idioma, que es
        // mejor que no mostrar nada.
        let en_aleman = parsear(COMPLETA, "x.desktop", "de", &["Vasak".to_string()]).unwrap();
        assert_eq!(en_aleman.nombre, "Web Browser");
    }

    #[test]
    fn las_acciones_son_resultados_propios() {
        let entrada = leer(COMPLETA).unwrap();

        assert_eq!(entrada.acciones.len(), 2);
        assert_eq!(entrada.acciones[0].nombre, "Nueva ventana privada");
        assert_eq!(entrada.acciones[0].exec, "firefox --private-window");
        assert_eq!(entrada.acciones[1].icono.as_deref(), Some("firefox-perfil"));
    }

    #[test]
    fn una_accion_declarada_y_no_definida_no_inventa_nada() {
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nActions=falta;\n";
        assert!(leer(contenido).unwrap().acciones.is_empty());
    }

    #[test]
    fn un_grupo_de_accion_que_nadie_declara_se_ignora() {
        // Queda en archivos viejos, y el estándar dice que no cuenta.
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=x\n\n[Desktop Action suelta]\nName=Suelta\nExec=y\n";
        assert!(leer(contenido).unwrap().acciones.is_empty());
    }

    #[test]
    fn las_claves_de_una_accion_no_pisan_las_de_la_entrada() {
        // Leyendo el archivo plano, el `Name` de la última acción terminaba
        // siendo el nombre de la aplicación.
        let entrada = leer(COMPLETA).unwrap();
        assert_eq!(entrada.nombre, "Navegador web");
    }

    #[test]
    fn lo_que_no_es_una_aplicacion_no_entra() {
        let enlace = "[Desktop Entry]\nType=Link\nName=X\nURL=https://x.test\n";
        assert!(leer(enlace).is_none());
    }

    #[test]
    fn las_entradas_auxiliares_no_entran() {
        // `NoDisplay` es la mitad de las entradas de un escritorio: manejadores
        // de protocolos, componentes, asistentes que abre otro programa.
        let oculta = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=true\n";
        assert!(leer(oculta).is_none());

        let borrada = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nHidden=true\n";
        assert!(leer(borrada).is_none());
    }

    #[test]
    fn solo_true_es_verdadero() {
        // `NoDisplay=1` o `NoDisplay=True` no son `true`. Tratarlos como sí
        // esconde aplicaciones que tienen que estar.
        let entrada = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNoDisplay=1\n";
        assert!(leer(entrada).is_some());
    }

    #[test]
    fn las_entradas_de_otros_escritorios_no_entran() {
        let ajena = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;KDE;\n";
        assert!(leer(ajena).is_none());

        let propia = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=GNOME;Vasak;\n";
        assert!(leer(propia).is_some());
    }

    #[test]
    fn y_las_que_nos_excluyen_tampoco() {
        let excluida = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNotShowIn=Vasak;\n";
        assert!(leer(excluida).is_none());

        let ajena = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nNotShowIn=GNOME;\n";
        assert!(leer(ajena).is_some());
    }

    #[test]
    fn el_nombre_del_escritorio_se_compara_sin_mayusculas() {
        // Nuestra sesión lo escribe de dos formas distintas según por dónde
        // arranque; una entrada no puede desaparecer por eso.
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nOnlyShowIn=vasak;\n";
        assert!(parsear(contenido, "x.desktop", "es", &["Vasak".into()]).is_some());
    }

    #[test]
    fn sin_nombre_o_sin_exec_no_hay_entrada() {
        let sin_nombre = "[Desktop Entry]\nType=Application\nExec=x\n";
        assert!(leer(sin_nombre).is_none());

        let sin_exec = "[Desktop Entry]\nType=Application\nName=X\n";
        assert!(leer(sin_exec).is_none());

        // Y una clave presente pero vacía es lo mismo que no tenerla: el
        // nombre en blanco deja una fila invisible en la lista.
        let vacia = "[Desktop Entry]\nType=Application\nName=\nExec=x\n";
        assert!(leer(vacia).is_none());
    }

    #[test]
    fn un_archivo_sin_el_grupo_principal_no_es_una_entrada() {
        let suelto = "[Otra Cosa]\nName=X\nExec=x\n";
        assert!(leer(suelto).is_none());
    }

    #[test]
    fn los_comentarios_y_las_lineas_sueltas_no_molestan() {
        let contenido = "# un comentario\n[Desktop Entry]\n\nType=Application\nName=X\nExec=x\nbasura sin igual\n";
        assert_eq!(leer(contenido).unwrap().nombre, "X");
    }

    #[test]
    fn un_valor_con_igual_adentro_se_conserva_entero() {
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=programa --opcion=valor\n";
        assert_eq!(leer(contenido).unwrap().exec, "programa --opcion=valor");
    }

    #[test]
    fn la_terminal_se_lee() {
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=htop\nTerminal=true\n";
        assert!(leer(contenido).unwrap().terminal);
    }

    #[test]
    fn sin_try_exec_la_entrada_se_puede_ejecutar() {
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=x\n";
        assert!(leer(contenido).unwrap().puede_ejecutarse());
    }

    #[test]
    fn un_try_exec_que_no_existe_deja_la_entrada_afuera() {
        // El programa desinstalado que deja su `.desktop`: aparece, se elige, y
        // no abre nada.
        let contenido =
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=no-existe-este-programa\n";
        assert!(!leer(contenido).unwrap().puede_ejecutarse());
    }

    #[test]
    fn un_try_exec_con_ruta_se_busca_como_ruta() {
        let contenido = "[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/bin/sh\n";
        assert!(leer(contenido).unwrap().puede_ejecutarse());

        let inexistente =
            "[Desktop Entry]\nType=Application\nName=X\nExec=x\nTryExec=/no/existe/nada\n";
        assert!(!leer(inexistente).unwrap().puede_ejecutarse());
    }
}
