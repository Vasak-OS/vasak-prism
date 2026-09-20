# Prism — el lanzador de VasakOS

Una consulta, todos los resultados: aplicaciones, archivos, acciones del
sistema, cuentas, ventanas abiertas. Se abre con `<super>`+espacio, se escribe,
se aprieta Enter.

Sale de lo que hasta ahora vivía adentro de `vasak-desktop` como una vista más.
Como aplicación propia puede crecer —archivos, calculadora, extensiones— y, sobre
todo, puede quedarse **residente**, que es lo único que la hace instantánea.

> **Estado: naciendo.** Ahora mismo esto es la plantilla `vapp` con el nombre
> puesto. Lo que falta está en los
> [issues del repositorio](https://github.com/Vasak-OS/vasak-prism/issues), en
> orden: parser de `.desktop`, índice de aplicaciones, daemon residente,
> interfaz, lanzamiento, y después los proveedores.

---

## Las tres decisiones que definen el programa

**El proceso no arranca cuando lo abrís.** Arranca con la sesión, por
`vasak-prism.service`, y la ventana se construye **oculta**. El atajo hace
`show()`, no `create()`. La búsqueda que había en el escritorio creaba y
destruía la ventana en cada apertura —en el `blur` hacía `close()`— y por eso
pagaba el arranque completo de un WebView cada vez. El objetivo es **menos de
80 ms** desde la tecla hasta la primera lista pintada, y **0 % de CPU** en
reposo: sin temporizadores, sólo inotify y el atajo.

**El índice está antes de la primera letra.** Las aplicaciones se parsean una
vez y quedan en `$XDG_CACHE_HOME/vasak-prism/`. El arranque tiene tres tiempos:
se lee la caché y con eso ya se puede contestar, se revalida en otro hilo, y de
ahí en adelante avisa inotify con coalescencia de ráfagas. Por reloj no: instalar
algo y esperar cinco minutos a que aparezca es lo que hacía la versión anterior,
y releía quinientos archivos seis veces por hora aunque no hubiera cambiado nada.

Los **iconos** son la otra mitad de esto, y no se guardan resueltos: son
megabytes y el tema se cambia en caliente. Lo que hay que resolver son los ocho
que se ven, memorizando por nombre. Está medido y anotado en el issue #15.

**Todo el procesamiento es de Rust.** El frontend pinta y navega; no busca, no
ordena, no lee el disco. Los proveedores corren en paralelo con un presupuesto de
unos 30 ms y **emiten resultados parciales**, así las aplicaciones aparecen en el
primer cuadro y los archivos entran cuando llegan. Cada consulta lleva un número
de generación: con resultados en streaming, una respuesta vieja pisa a una nueva
si no se controla.

---

## Correrlo

```bash
bun install
bun test                                          # frontend
cargo test --manifest-path src-tauri/Cargo.toml   # backend
bunx --bun tauri dev
```

Para probar la ventana compilada hace falta `--features custom-protocol`, o el
WebView abre vacío.

---

## Cómo está armado

| carpeta | qué hay |
|---|---|
| `src/` | la interfaz, con `@vasakgroup/vue-libvasak` |
| `src-tauri/src/` | el índice, los proveedores, el ranking y el daemon |
| `src-tauri/locales/` | los catálogos de idioma, uno por idioma |
| `packaging/` | la entrada del escritorio, y más adelante la unidad de systemd |
| `tests/`, `src-tauri/tests/` | las pruebas de cada lado |

El nombre del programa aparece en cinco archivos que no dependen entre sí
(`package.json`, `Cargo.toml`, `tauri.conf.json`, `index.html`, `main.rs`) más la
entrada del escritorio. Hay una prueba que los ata: si alguien renombra algo y se
olvida de uno, `bun test` lo dice — nada de eso rompe la compilación, y el
síntoma aparecería recién en una máquina con el paquete instalado.

El resto de las convenciones —el `slot` de `WindowAppLayout`, los iconos
reactivos, por qué la ruta de los catálogos va explícita— son las de la
plantilla, y están documentadas en [vapp](https://github.com/Vasak-OS/vapp).
