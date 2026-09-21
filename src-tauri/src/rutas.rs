//! Las bases del estándar, comprobadas.
//!
//! `dirs` resuelve `XDG_CACHE_HOME`, `XDG_CONFIG_HOME` y `XDG_DATA_HOME` como
//! manda el estándar: una variable relativa se **ignora** y se cae al respaldo,
//! y la cadena vacía es un caso de esa misma regla porque tampoco es absoluta.
//! Lo que `dirs` **no** comprueba es el respaldo: de `HOME` sólo mira que no
//! esté vacía, así que un `HOME` relativo vuelve como base relativa.
//!
//! Esta es esa otra mitad, escrita una vez. Antes el crate tenía cinco copias
//! del mismo filtro —cache, frecuencia, escaneo, recientes y configuración—,
//! que es exactamente como cinco copias se separan: cuatro se arreglan y una
//! queda.
//!
//! Importa por lo que pasa si no está: una base relativa se resuelve contra el
//! directorio de trabajo, y Prism corre como daemon de systemd, donde eso no es
//! el home de nadie. Ni la caché ni la frecuencia de uso ni los recientes
//! estarían donde se los busca, y nada fallaría.

use std::path::PathBuf;

/// La base, sólo si es absoluta.
///
/// Se le pasa `dirs::cache_dir()`, `dirs::data_dir()` o la que corresponda.
/// Devolver `None` y no adivinar es a propósito: quien llama ya sabe qué hacer
/// sin base —la caché no se guarda, los recientes no aparecen— y escribir en un
/// directorio que nadie eligió es peor que no escribir.
pub fn base(cual: Option<PathBuf>) -> Option<PathBuf> {
    cual.filter(|base| base.is_absolute())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_base_absoluta_pasa() {
        assert_eq!(
            base(Some(PathBuf::from("/home/pato/.cache"))),
            Some(PathBuf::from("/home/pato/.cache"))
        );
    }

    #[test]
    fn una_base_relativa_no_pasa() {
        // Las cuatro formas de no ser absoluta. La del nombre suelto es la que
        // se escapa cuando uno se acuerda sólo de la vacía, que es el error que
        // tenían las cinco copias que esto reemplaza.
        for relativa in ["", "cache", "./cache", "../cache"] {
            assert_eq!(
                base(Some(PathBuf::from(relativa))),
                None,
                "«{relativa}» no es absoluta"
            );
        }
    }

    #[test]
    fn sin_base_no_hay_base() {
        assert_eq!(base(None), None);
    }
}
