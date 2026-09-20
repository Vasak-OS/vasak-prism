//! El desarmado de la línea `Exec` de una entrada del escritorio.
//!
//! Vive aparte porque es la parte que más se rompe y la única que se puede
//! probar entera sin tocar el disco. Lo que había en el escritorio sacaba los
//! códigos de campo con un `replace` por cada uno y después partía por espacios
//! en blanco: `Exec=sh -c "algo con espacios"` quedaba en cinco argumentos y no
//! arrancaba nada, y `%%` —un porcentaje literal— desaparecía.

/// Desarma una línea `Exec` en programa y argumentos, según la especificación.
///
/// `nombre` e `icono` son los de la entrada, para `%c` y `%i`; `ruta` es la del
/// archivo `.desktop`, para `%k`. Devuelve `None` si no queda ningún argumento:
/// una entrada sin programa no se puede lanzar.
pub fn desarmar(linea: &str, nombre: &str, icono: Option<&str>, ruta: &str) -> Option<Vec<String>> {
    let mut salida = Vec::new();

    for bruto in trocear(linea)? {
        // `%i` es el único que se expande a **dos** argumentos, así que se
        // resuelve antes de tocar el resto del texto.
        if bruto == "%i" {
            if let Some(icono) = icono {
                salida.push("--icon".to_string());
                salida.push(icono.to_string());
            }
            continue;
        }

        if let Some(arg) = expandir(&bruto, nombre, ruta) {
            salida.push(arg);
        }
    }

    (!salida.is_empty()).then_some(salida)
}

/// Parte la línea en argumentos respetando las comillas.
///
/// Las reglas son las del estándar: comillas dobles, y adentro la barra
/// invertida escapa `\`, `"`, `` ` `` y `$`. Devuelve `None` si una comilla
/// queda abierta — eso es un archivo mal escrito, y lanzar «lo que se entendió»
/// de una línea así es peor que no lanzar nada.
fn trocear(linea: &str) -> Option<Vec<String>> {
    let mut salida = Vec::new();
    let mut actual = String::new();
    let mut hay_argumento = false;
    let mut entre_comillas = false;
    let mut caracteres = linea.chars();

    while let Some(c) = caracteres.next() {
        match c {
            '"' => {
                entre_comillas = !entre_comillas;
                // Unas comillas vacías son un argumento vacío, no la ausencia
                // de argumento: `foo "" bar` son tres.
                hay_argumento = true;
            }
            '\\' => {
                // Una barra al final de la línea no escapa nada: el archivo
                // está cortado, y vale lo mismo que una comilla sin cerrar.
                let siguiente = caracteres.next()?;
                // Fuera de comillas la barra escapa igual: es como el estándar
                // pide que se escriban los caracteres reservados.
                actual.push(siguiente);
                hay_argumento = true;
            }
            c if c.is_whitespace() && !entre_comillas => {
                if hay_argumento {
                    salida.push(std::mem::take(&mut actual));
                    hay_argumento = false;
                }
            }
            c => {
                actual.push(c);
                hay_argumento = true;
            }
        }
    }

    if entre_comillas {
        return None;
    }

    if hay_argumento {
        salida.push(actual);
    }

    Some(salida)
}

/// Resuelve los códigos de campo de un argumento ya troceado.
///
/// Devuelve `None` cuando el argumento entero era un código que no aplica: un
/// `%F` suelto se saca, no se deja como cadena vacía, porque un argumento vacío
/// de más cambia lo que recibe el programa.
fn expandir(argumento: &str, nombre: &str, ruta: &str) -> Option<String> {
    if !argumento.contains('%') {
        return Some(argumento.to_string());
    }

    let mut salida = String::new();
    let mut caracteres = argumento.chars().peekable();

    while let Some(c) = caracteres.next() {
        if c != '%' {
            salida.push(c);
            continue;
        }

        match caracteres.next() {
            // El único que deja texto: un porcentaje de verdad.
            Some('%') => salida.push('%'),
            // Los que se refieren a archivos o URL que acá no hay: se van.
            // `%d %D %n %N %v %m` están obsoletos desde hace más de una década
            // y el estándar pide ignorarlos, no pasarlos.
            Some('f' | 'F' | 'u' | 'U' | 'd' | 'D' | 'n' | 'N' | 'v' | 'm') => {}
            Some('c') => salida.push_str(nombre),
            Some('k') => salida.push_str(ruta),
            // `%i` se resuelve antes de llegar acá, y cualquier otro código es
            // un error de la entrada: se ignora en lugar de pasarlo crudo al
            // programa, que es lo que hace glib.
            Some(_) | None => {}
        }
    }

    (!salida.is_empty()).then_some(salida)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(linea: &str) -> Option<Vec<String>> {
        desarmar(linea, "Programa", None, "/usr/share/applications/x.desktop")
    }

    #[test]
    fn una_linea_normal_se_parte_en_programa_y_argumentos() {
        assert_eq!(
            simple("firefox --new-window"),
            Some(vec!["firefox".into(), "--new-window".into()])
        );
    }

    #[test]
    fn las_comillas_mantienen_junto_lo_que_lleva_espacios() {
        // El caso que rompía el escritorio: partir por espacios en blanco deja
        // `sh -c echo hola` en cuatro argumentos y `sh` no ejecuta nada.
        assert_eq!(
            simple(r#"sh -c "echo hola""#),
            Some(vec!["sh".into(), "-c".into(), "echo hola".into()])
        );
    }

    #[test]
    fn una_ruta_con_espacios_sigue_siendo_una_ruta() {
        assert_eq!(
            simple(r#""/opt/Mi Programa/bin/correr" --modo=rapido"#),
            Some(vec![
                "/opt/Mi Programa/bin/correr".into(),
                "--modo=rapido".into()
            ])
        );
    }

    #[test]
    fn la_barra_invertida_escapa_adentro_de_las_comillas() {
        assert_eq!(
            simple(r#"programa "una \"cita\" adentro""#),
            Some(vec!["programa".into(), r#"una "cita" adentro"#.into()])
        );
    }

    #[test]
    fn una_comilla_sin_cerrar_no_se_lanza_a_medias() {
        // Mejor no abrir nada que abrir otra cosa: una línea mal escrita con
        // «lo que se entendió» puede ser un comando distinto del que dice.
        assert_eq!(simple(r#"programa "sin cerrar"#), None);
    }

    #[test]
    fn los_codigos_de_archivo_se_sacan_enteros() {
        // Y se sacan como argumento: dejar la cadena vacía le pasa al programa
        // un argumento de más, que para muchos significa «abrí esto».
        assert_eq!(simple("gimp %U"), Some(vec!["gimp".into()]));
        assert_eq!(
            simple("okular %F --presentation"),
            Some(vec!["okular".into(), "--presentation".into()])
        );
    }

    #[test]
    fn el_porcentaje_literal_sobrevive() {
        // Lo que perdía el `replace` de a uno: `%%` es un porcentaje, no un
        // código de campo, y hay entradas que lo usan en un formato.
        assert_eq!(
            simple("medir --formato=100%%"),
            Some(vec!["medir".into(), "--formato=100%".into()])
        );
    }

    #[test]
    fn el_icono_se_expande_a_dos_argumentos() {
        assert_eq!(
            desarmar("programa %i", "Programa", Some("mi-icono"), "/x.desktop"),
            Some(vec!["programa".into(), "--icon".into(), "mi-icono".into()])
        );
    }

    #[test]
    fn y_sin_icono_no_deja_el_argumento_colgado() {
        // `--icon` sin valor hace que muchos programas no arranquen.
        assert_eq!(simple("programa %i"), Some(vec!["programa".into()]));
    }

    #[test]
    fn el_nombre_y_la_ruta_se_ponen_donde_se_piden() {
        assert_eq!(
            desarmar("programa %c %k", "Mi Programa", None, "/ruta/x.desktop"),
            Some(vec![
                "programa".into(),
                "Mi Programa".into(),
                "/ruta/x.desktop".into()
            ])
        );
    }

    #[test]
    fn un_codigo_pegado_a_otro_texto_deja_el_texto() {
        // Es como se comporta glib, y hay entradas reales que lo escriben así.
        assert_eq!(
            simple("visor --archivo=%f"),
            Some(vec!["visor".into(), "--archivo=".into()])
        );
    }

    #[test]
    fn una_linea_vacia_no_da_programa() {
        assert_eq!(simple(""), None);
        assert_eq!(simple("   "), None);
        assert_eq!(simple("%F"), None);
    }
}
