//! El catálogo de aplicaciones: leerlas, guardarlas, mantenerlas al día y
//! buscarlas.
//!
//! El arranque tiene tres tiempos y el orden es deliberado:
//!
//!  1. **Se lee la caché** y con eso ya se puede contestar. Es una consulta a
//!     una base de unos cientos de kilobytes contra leer y parsear quinientos
//!     archivos: la diferencia se nota en la primera consulta después de
//!     prender la máquina, que es justo cuando el disco está frío.
//!  2. **Se revalida en otro hilo.** Si algo cambió, se reindexa y se avisa.
//!  3. **Queda inotify.** De ahí en adelante el disco avisa y nadie sondea.

pub mod aplicacion;
pub mod cache;
pub mod entrada;
pub mod escaneo;
pub mod exec;
pub mod frecuencia;
pub mod puntaje;
pub mod vigilancia;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use aplicacion::{Aplicacion, Resultado};

/// El escritorio en el que corremos, para `OnlyShowIn` y `NotShowIn`.
///
/// Sale de `XDG_CURRENT_DESKTOP`, que es una lista separada por dos puntos. Si
/// no está, se asume el nuestro: es lo más probable, y la alternativa es una
/// lista vacía, con la que toda entrada que use `OnlyShowIn` desaparece.
pub fn escritorios_actuales() -> Vec<String> {
    escritorios_de(&std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default())
}

/// Separado de la lectura de la variable para poder probarlo: tocar el entorno
/// adentro de una prueba se lo toca a las que corren en paralelo.
pub fn escritorios_de(valor: &str) -> Vec<String> {
    let lista: Vec<String> = valor
        .split(':')
        .map(str::trim)
        .filter(|parte| !parte.is_empty())
        .map(str::to_string)
        .collect();

    if lista.is_empty() {
        vec!["Vasak".to_string()]
    } else {
        lista
    }
}

/// Las aplicaciones que se pueden lanzar, y con qué buscarlas.
///
/// La lista se reemplaza entera al reindexar, no se modifica en su lugar: quien
/// está en medio de una búsqueda sigue con la que tenía, y no hay un instante
/// en el que el catálogo esté a medio armar.
pub struct Catalogo {
    aplicaciones: RwLock<Arc<Vec<Aplicacion>>>,
    idioma: String,
    escritorios: Vec<String>,
    ruta_cache: Option<PathBuf>,
}

impl Catalogo {
    pub fn nuevo(idioma: String, escritorios: Vec<String>, ruta_cache: Option<PathBuf>) -> Self {
        Self {
            aplicaciones: RwLock::new(Arc::new(Vec::new())),
            idioma,
            escritorios,
            ruta_cache,
        }
    }

    /// Lo que hay ahora. Un `Arc` para que buscar no tenga tomado el candado
    /// mientras puntúa: reindexar no puede quedar esperando a una búsqueda.
    pub fn aplicaciones(&self) -> Arc<Vec<Aplicacion>> {
        Arc::clone(&self.aplicaciones.read().unwrap_or_else(|e| e.into_inner()))
    }

    fn reemplazar(&self, aplicaciones: Vec<Aplicacion>) {
        let mut guardadas = self.aplicaciones.write().unwrap_or_else(|e| e.into_inner());
        *guardadas = Arc::new(aplicaciones);
    }

    /// Carga lo guardado. Devuelve cuántas quedaron.
    ///
    /// Sin caché o con una caché ilegible devuelve cero y no falla: se rehace
    /// sola en la revalidación, que es el paso siguiente.
    pub fn cargar_de_cache(&self) -> usize {
        let Some(ruta) = &self.ruta_cache else {
            return 0;
        };
        let Ok(cache) = cache::Cache::abrir(ruta) else {
            return 0;
        };

        let guardadas = cache.leer();
        let cantidad = guardadas.len();
        self.reemplazar(guardadas);
        cantidad
    }

    /// Si lo que hay en memoria sigue valiendo para lo que hay en el disco.
    pub fn esta_al_dia(&self) -> bool {
        cache::esta_al_dia(&self.aplicaciones(), &escaneo::archivos())
    }

    /// Lee el disco, reemplaza la lista y guarda la caché.
    ///
    /// Devuelve `true` si la lista cambió. Lo mira quien avisa a la interfaz:
    /// un reindexado que da lo mismo que había no es novedad para nadie.
    pub fn reindexar(&self) -> bool {
        let nuevas = escaneo::escanear(&self.idioma, &self.escritorios);
        let cambio = *self.aplicaciones() != nuevas;

        if let Some(ruta) = &self.ruta_cache {
            if let Ok(mut cache) = cache::Cache::abrir(ruta) {
                // Que no se pueda guardar la caché no es motivo para no tener el
                // índice: se pierde el atajo del próximo arranque, nada más.
                let _ = cache.guardar(&nuevas);
            }
        }

        self.reemplazar(nuevas);
        cambio
    }

    pub fn buscar(&self, consulta: &str, limite: usize) -> Vec<Resultado> {
        buscar(&self.aplicaciones(), consulta, limite)
    }

    pub fn buscar_con_uso(
        &self,
        consulta: &str,
        limite: usize,
        pesos: &HashMap<String, f64>,
    ) -> Vec<Resultado> {
        buscar_con_uso(&self.aplicaciones(), consulta, limite, pesos)
    }
}

/// Los mejores `limite` resultados para la consulta.
///
/// Función suelta y no método para poder probarla con una lista escrita a mano,
/// que es lo único que hace falta para comprobar el orden.
pub fn buscar(aplicaciones: &[Aplicacion], consulta: &str, limite: usize) -> Vec<Resultado> {
    buscar_con_uso(aplicaciones, consulta, limite, &HashMap::new())
}

/// Igual, pero levantando lo que se usa seguido.
///
/// El empuje se aplica **antes** de recortar a `limite`: al revés, lo más usado
/// tendría que entrar primero entre los mejores por texto para poder subir, que
/// es justo cuando no hace falta.
pub fn buscar_con_uso(
    aplicaciones: &[Aplicacion],
    consulta: &str,
    limite: usize,
    pesos: &HashMap<String, f64>,
) -> Vec<Resultado> {
    if consulta.trim().is_empty() {
        return Vec::new();
    }

    let mut resultados: Vec<Resultado> = aplicaciones
        .iter()
        .flat_map(|app| aplicacion::resultados(app, consulta))
        .map(|mut fila| {
            let clave = frecuencia::clave(&fila.id, fila.accion.as_deref());
            let peso = pesos.get(&clave).copied().unwrap_or(0.0);
            fila.puntaje = frecuencia::con_uso(fila.puntaje, peso);
            fila
        })
        .collect();

    // Por puntaje, y a igual puntaje por título: sin el desempate, dos
    // aplicaciones que puntúan igual se turnan el primer puesto entre consulta y
    // consulta —el orden de un `HashMap` no es orden— y el resultado se mueve
    // abajo del dedo justo cuando se aprieta Enter.
    resultados.sort_by(|a, b| {
        b.puntaje
            .partial_cmp(&a.puntaje)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.titulo.cmp(&b.titulo))
    });
    resultados.truncate(limite);
    resultados
}

#[cfg(test)]
mod tests {
    use super::*;

    fn una(nombre: &str) -> Aplicacion {
        Aplicacion {
            id: format!("{nombre}.desktop"),
            nombre: nombre.to_string(),
            generico: None,
            comentario: None,
            palabras: Vec::new(),
            icono: None,
            exec: nombre.to_string(),
            terminal: false,
            acciones: Vec::new(),
            ruta: format!("/usr/share/applications/{nombre}.desktop"),
            mtime: 0,
        }
    }

    #[test]
    fn una_consulta_vacia_no_devuelve_todo() {
        // El lanzador abre con el cuadro vacío: devolver el catálogo entero es
        // pintar quinientas filas que nadie pidió.
        let apps = [una("Firefox"), una("Konsole")];
        assert!(buscar(&apps, "", 10).is_empty());
        assert!(buscar(&apps, "   ", 10).is_empty());
    }

    #[test]
    fn lo_que_mas_se_parece_va_primero() {
        let apps = [una("Firefox Developer Edition"), una("Firefox")];
        let filas = buscar(&apps, "firefox", 10);

        assert_eq!(filas[0].titulo, "Firefox");
    }

    #[test]
    fn el_limite_se_respeta() {
        let apps = [una("Archivo uno"), una("Archivo dos"), una("Archivo tres")];
        assert_eq!(buscar(&apps, "archivo", 2).len(), 2);
    }

    #[test]
    fn a_igual_puntaje_el_orden_es_siempre_el_mismo() {
        // Sin desempate, dos que puntúan igual se turnan el primer puesto y el
        // resultado se mueve abajo del dedo justo al apretar Enter.
        let apps = [una("Bbb"), una("Aaa")];
        let primera = buscar(&apps, "a", 10);
        let segunda = buscar(&apps, "a", 10);

        assert_eq!(primera, segunda);
        assert_eq!(primera[0].titulo, "Aaa");
    }

    #[test]
    fn lo_que_se_usa_seguido_sube() {
        let apps = [una("Archivo uno"), una("Archivo dos")];
        let mut pesos = HashMap::new();
        pesos.insert("Archivo dos.desktop".to_string(), 1.0);

        let filas = buscar_con_uso(&apps, "archivo", 10, &pesos);

        assert_eq!(filas[0].titulo, "Archivo dos");
    }

    #[test]
    fn el_empuje_se_aplica_antes_de_recortar() {
        // Al revés, lo más usado tendría que entrar primero entre los mejores
        // por texto para poder subir — que es justo cuando ya no hace falta.
        let apps = [una("Archivo a"), una("Archivo b"), una("Archivo c")];
        let mut pesos = HashMap::new();
        pesos.insert("Archivo c.desktop".to_string(), 1.0);

        let filas = buscar_con_uso(&apps, "archivo", 1, &pesos);

        assert_eq!(filas.len(), 1);
        assert_eq!(filas[0].titulo, "Archivo c");
    }

    #[test]
    fn sin_xdg_current_desktop_se_asume_el_nuestro() {
        // Con una lista vacía, toda entrada que use `OnlyShowIn` desaparece.
        let lista = escritorios_de("");
        assert_eq!(lista, vec!["Vasak".to_string()]);
    }

    #[test]
    fn el_escritorio_es_una_lista() {
        assert_eq!(
            escritorios_de("Vasak:wlroots"),
            vec!["Vasak".to_string(), "wlroots".to_string()]
        );
    }
}
