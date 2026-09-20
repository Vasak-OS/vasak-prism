# vapp — plantilla de aplicaciones de VasakOS

Punto de partida para una aplicación del escritorio: Tauri 2 + Vue 3 +
TypeScript + Tailwind 4, con los cuatro plugins de VasakOS ya enchufados y con
las decisiones que las aplicaciones reales tuvieron que aprender rompiéndose.

**Lo que hay acá no es andamiaje de relleno.** Cada pieza está porque su ausencia
causó un bug concreto en `vasak-resonance`, `vasak-terminal`, `vasak-gallery` o
`vasak-desktop`. Los comentarios del código dicen cuál.

---

## Arrancar una aplicación nueva

```bash
git clone https://github.com/Vasak-OS/vapp mi-app && cd mi-app && rm -rf .git && git init
```

Después, cambiar el nombre en **cinco lugares** —y son cinco, no uno:

| archivo | qué cambiar |
|---|---|
| `package.json` | `name` |
| `src-tauri/Cargo.toml` | `name`, `[lib] name` (`mi_app_lib`), `description` |
| `src-tauri/tauri.conf.json` | `productName`, `identifier` (`ar.net.vasak.mi-app`) |
| `index.html` | `<title>` |
| `src-tauri/src/main.rs` | la llamada a `mi_app_lib::run()` |

El `identifier` decide dónde van la configuración y los datos del usuario:
cambiarlo después de la primera ejecución deja huérfano lo que ya se guardó.

Y **verificá que los tests pasen antes de escribir nada**: si el `name` quedó a
medio cambiar, el test de catálogos lo dice enseguida.

```bash
bun install
bun test                                   # frontend
cargo test --manifest-path src-tauri/Cargo.toml   # backend
bunx --bun tauri dev
```

---

## Lo que ya viene resuelto, y por qué

### El marco de la ventana

`WindowAppLayout` recibe el contenido en un `slot`, así que la aplicación va
adentro:

```vue
<WindowAppLayout>
  <MiContenido />
</WindowAppLayout>
```

Sin ese `slot` —que es como estaba— el layout **descartaba en silencio** todo lo
que se le pusiera dentro y la ventana abría vacía con el relleno de la plantilla
todavía puesto. No hay ningún error: simplemente no aparece nada, y en
vasak-monitor costó una compilación y una captura darse cuenta.

### Idioma de la sesión

`src-tauri/src/locales.rs` resuelve dos cosas que se rompen calladas:

**Dónde están los catálogos.** El plugin sólo prueba rutas relativas al
ejecutable y al directorio de trabajo, y **ninguna existe cuando el binario está
en `/usr/bin`**. Sin la ruta explícita, un paquete instalado muestra las claves
crudas (`inicio.titulo`) en lugar de los textos. Le pasó al gestor de archivos, a
la terminal y a la galería.

Al empaquetar, el PKGBUILD tiene que instalar los `.yml`:

```bash
install -dm755 "${pkgdir}/usr/share/${pkgname}/locales"
install -Dm644 "$srcdir/$pkgname/src-tauri/locales/"*.yml \
    "${pkgdir}/usr/share/${pkgname}/locales/"
```

**Qué idioma usar.** Se recorren `LC_ALL`, `LC_MESSAGES` y `LANG` en ese orden,
**salteando las vacías**: `LC_ALL=""` junto a `LANG=en_US.UTF-8` es una máquina en
inglés, y quedarse con la vacía la dejaría en español.

### Escribir textos traducidos

Todo texto que ve una persona va en los `.yml`. Nunca una cadena literal en un
`.vue`.

El `t()` del plugin toma **un solo argumento y no interpola**. La convención es el
marcador `{0}` y `src/tools/interpolar.ts`:

```ts
import { interpolar } from '@/tools/interpolar';
interpolar(t('inicio.saludo'), nombre);
```

**Usá el ayudante, no `replace` a mano.** `String.prototype.replace` interpreta
`$&`, `$$`, `` $` `` y `$'` en la cadena de reemplazo: una canción llamada
«Rock $& Roll» se mostraba como «Rock {0} Roll», y una con `$'` **perdía el texto
que venía después**. Hay tests con los tres casos.

Y no hay plurales: van dos claves con sufijo `One`/`Other` y la vista elige con
`claveSegunCantidad()`. Sin eso se termina mostrando «1 pistas».

Dos reglas de los `.yml`, las dos con test:

1. **Si el valor contiene `: `, va entre comillas.** Sin ellas el parser lo lee
   como un mapeo anidado, rompe el archivo entero y el plugin **paniquea al
   arrancar**: la aplicación no abre. Pasó en producción.
2. **Los idiomas tienen las mismas claves y los mismos marcadores.** Una clave que
   falta se muestra cruda; un `{0}` que está en uno y no en el otro pierde el
   dato.

### Política de contenido

`tauri.conf.json` trae una CSP con `script-src 'self'`, que es lo que bloquea la
inyección de script. Si tu aplicación necesita algo más —una API externa en
`connect-src`, `blob:` en `img-src`— agregalo ahí, **no la aflojes entera**.

`style-src` lleva `'unsafe-inline'` a propósito: Vue escribe estilos en línea con
`:style` y Tailwind inyecta los suyos. Esta política cubre script, no estilo.

`main.ts` registra un reportador de violaciones. Hace falta porque **una violación
de CSP no se ve**: el recurso no carga y la interfaz queda a medias sin decir
nada. El reportador sanea las URLs antes de escribirlas —credenciales, query y
fragmento fuera— porque ahí viajan tokens.

Cuidado con las rutas absolutas de archivos: `file://` **no** está permitido, y
está bien que no lo esté. Usá `convertFileSrc()`, que devuelve una URL del
protocolo de assets.

### Clic derecho

`setupContextMenu({ iconResolver: getIconSource })` en `main.ts`. Sin eso, el
clic derecho abre el menú del motor del navegador, con «Recargar» e «Inspeccionar
elemento» — visiblemente ajeno al escritorio.

### Tema y colores

`useConfigStore()` del plugin de configuración, y `App.vue` escucha
`config-changed` para que el cambio de tema se aplique sin reiniciar. Los colores
salen de variables CSS (`--use-*`), no de valores fijos.

### Iconos

`useReactiveIcon()` en `src/composables/`. Reactivo porque el pack de iconos
puede cambiar en caliente.

---

## Tests

**Cada cambio va con tests, en el mismo commit.** Y si el repo tiene poca
cobertura, se suman algunos de lo que está alrededor: así sube mientras se
avanza, en lugar de necesitar una campaña de testing que nunca llega.

Lo que conviene probar es **lo que se rompe callado**: parsers, recortes de
texto, límites, entradas mal formadas, plurales, marcadores de interpolación. No
la interfaz.

La plantilla trae los dos lados armados:

- `tests/interpolar.test.ts` — la interpolación, con los tres casos del `$`.
- `src-tauri/tests/locales.rs` — que los catálogos parseen, tengan las mismas
  claves, ningún texto vacío y los marcadores coincidan. Recorre el árbol
  **completo**: cuando sólo bajaba dos niveles, un grupo anidado más abajo quedaba
  sin comprobar y encima aparecía como texto vacío, porque un mapeo no es una
  cadena — el test fallaba justo donde no miraba.
- `src-tauri/src/locales.rs` — la detección de idioma.

**Verificá que un test sirve reintroduciendo el bug a propósito** y viendo que
falle. Un test que pasa siempre no prueba nada.

Dos trampas al extraer lógica para poder probarla:

- Separá el parseo de la I/O. Ese refactor es el que suele destapar el bug —así
  apareció un panic por recortar UTF-8 por bytes en la terminal.
- No dejes un `catch` vacío. Tragarse el error hace imposible saber si la llamada
  llegó, y cuesta horas de diagnóstico.

---

## Compilar y empaquetar

```bash
bun run lint          # biome
bun test
cargo test --manifest-path src-tauri/Cargo.toml
bunx --bun tauri build
```

**Compilá siempre con `tauri build`, nunca con `cargo build --release` a secas.**
Con `cargo` el binario queda apuntando al servidor de desarrollo: la página
«carga» vacía, no ejecuta nada de JavaScript, y todo parece roto por otra razón.
Perdí una tarde con eso.

El PKGBUILD necesita, además de los `.yml` de arriba:

- `options=('!lto')` — makepkg inyecta `-flto` global y los crates que compilan C
  o assembly emiten bitcode que el enlazador de rustc no resuelve. Cargo ya hace
  su propio LTO.
- `RUSTFLAGS="-C target-cpu=x86-64"` y `unset CARGO_ENCODED_RUSTFLAGS` — el
  `target-cpu=native` de la máquina que compila produce binarios que mueren con
  SIGILL en cualquier CPU más vieja.

---

## Rendimiento: lo que aprendimos a no hacer

Estas aplicaciones corren en el escritorio de alguien, y su JavaScript comparte
hilo con el dibujado. Un cálculo de 30 ms son dos cuadros perdidos.

- **No sondees si podés escuchar.** Y si tenés que sondear, pausá con
  `document.hidden`: nadie lee una pantalla que no está en pantalla.
- **Un temporizador, no N.** Un `setInterval` por elemento se multiplica sin que
  se note.
- **Un reloj sin segundos despierta al minuto**, no a 1 Hz. 59 de cada 60
  despertares no cambian un píxel, y `toLocaleTimeString` no es gratis.
- **Lo pesado va en Rust.** Decodificar imágenes o video, hashear, recorrer
  árboles de archivos: en el backend, y que cruce el IPC una ruta, no los bytes.
- **No copies para notificar.** Un `{ ...objeto, clave: valor }` sobre un `ref`
  reactivo copia todo el objeto en cada cambio; asignar la clave ya notifica.

---

## Estructura

```
src/
  App.vue                 tema y configuración; el marco de la ventana
  main.ts                 plugins, CSP, i18n, menú contextual
  assets/main.css         Tailwind y las variables del tema
  components/topbar/      la barra de título propia (la ventana no tiene decoración)
  composables/            useReactiveIcon y compañía
  layouts/                WindowAppLayout
  tools/interpolar.ts     interpolación y plurales de textos traducidos
tests/                    tests del frontend (bun test)
src-tauri/
  src/lib.rs              registro de plugins
  src/locales.rs          resolución de idioma y de la ruta de catálogos
  locales/{es,en}.yml     los textos
  tests/locales.rs        validación de los catálogos
  capabilities/           permisos por ventana
  tauri.conf.json         ventana, CSP, empaquetado
```

## Licencia

GPL-3.0-or-later.
