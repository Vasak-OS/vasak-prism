/**
 * Lo que decide cómo entra Prism al escritorio.
 *
 * Tres archivos que no se compilan y que por eso se rompen callados: la lista de
 * iconos de `tauri.conf.json`, la entrada del escritorio, y lo que las ata al
 * nombre del binario. Nada de esto lo mira el compilador ni `vue-tsc`, y el
 * síntoma aparece recién en una máquina con el paquete instalado: un icono
 * genérico, una entrada que no abre nada, o una ventana que el escritorio no
 * reconoce como abierta.
 *
 * En la tienda ya pasó de las dos formas: `bundle.icon` siguió nombrando cuatro
 * archivos borrados, y el `StartupWMClass` decía `vasak-store`, que no es el
 * identificador de nada.
 */

import { describe, expect, test } from 'bun:test';

const RAIZ = new URL('..', import.meta.url);
const ENTRADA = 'packaging/vasak-prism.desktop';

const configuracion = JSON.parse(
	await Bun.file(new URL('src-tauri/tauri.conf.json', RAIZ)).text()
);
const entrada = await Bun.file(new URL(ENTRADA, RAIZ)).text();

/** Las claves del `.desktop`, sin los comentarios. */
const claves = new Map(
	entrada
		.split('\n')
		.filter((linea) => !linea.startsWith('#') && linea.includes('='))
		.map((linea) => {
			const corte = linea.indexOf('=');
			return [linea.slice(0, corte), linea.slice(corte + 1)];
		})
);

describe('el nombre del programa', () => {
	test('es el mismo en los manifiestos que se empaquetan', async () => {
		// El renombre desde la plantilla toca cinco archivos y ninguno depende
		// del otro: quedarse a medio camino compila igual. Lo que no compila
		// igual es el paquete.
		const paquete = JSON.parse(await Bun.file(new URL('package.json', RAIZ)).text());
		const cargo = await Bun.file(new URL('src-tauri/Cargo.toml', RAIZ)).text();

		expect(paquete.name).toBe('vasak-prism');
		expect(configuracion.productName).toBe('vasak-prism');
		expect(cargo).toContain('name = "vasak-prism"');
		// El `_lib` con guiones bajos, que es como lo nombra `main.rs`.
		expect(cargo).toContain('name = "vasak_prism_lib"');
	});

	test('y la versión también', async () => {
		// Los manifiestos que compara la CI. Un número distinto entre ellos deja
		// un paquete que dice una versión y trae otra.
		const paquete = JSON.parse(await Bun.file(new URL('package.json', RAIZ)).text());
		const cargo = await Bun.file(new URL('src-tauri/Cargo.toml', RAIZ)).text();

		expect(configuracion.version).toBe(paquete.version);
		expect(cargo).toContain(`version = "${paquete.version}"`);
	});

	test('el identificador es el del ecosistema', () => {
		// De él salen el directorio de configuración y el de datos del usuario.
		// Cambiarlo después de la primera ejecución deja huérfano lo guardado.
		expect(configuracion.identifier).toBe('ar.net.vasak.prism');
	});
});

describe('los iconos que se empaquetan', () => {
	test('cada uno de los que se nombran existe', async () => {
		const nombrados: string[] = configuracion.bundle.icon;
		expect(nombrados.length).toBeGreaterThan(0);

		for (const relativo of nombrados) {
			const archivo = Bun.file(new URL(`src-tauri/${relativo}`, RAIZ));
			expect(await archivo.exists(), `${relativo} no existe`).toBe(true);
		}
	});

	test('y no queda ninguno suelto que nadie nombre', async () => {
		// Al revés también: un icono que no está en la lista es peso muerto que
		// se copia y se versiona. La plantilla traía dieciséis, entre ellos los
		// de Windows y los de macOS, en una aplicación que sólo corre en Linux.
		const nombrados: string[] = configuracion.bundle.icon;
		const iconos = new Bun.Glob('src-tauri/icons/*');
		const sueltos: string[] = [];
		for await (const ruta of iconos.scan({ cwd: RAIZ.pathname })) {
			if (!nombrados.includes(ruta.replace('src-tauri/', ''))) {
				sueltos.push(ruta);
			}
		}
		expect(sueltos).toEqual([]);
	});

	test('el que se empaqueta es un PNG, que es lo que el tema hicolor entiende', () => {
		// El empaquetador saca el tamaño de la imagen y la instala en
		// `hicolor/<ancho>x<alto>/apps/`. Un `.ico` o un `.icns` ahí no los
		// dibuja nadie en Linux.
		for (const relativo of configuracion.bundle.icon as string[]) {
			expect(relativo).toEndWith('.png');
		}
	});
});

describe('la entrada del escritorio', () => {
	test('abre el binario que el paquete instala', () => {
		expect(claves.get('Exec')).toBe(configuracion.productName);
	});

	test('pide el icono por el nombre con el que se instala', () => {
		// El empaquetador de Tauri nombra el PNG del tema hicolor como el
		// `productName`. Cualquier otro nombre acá cae en lo que el tema tenga
		// —o en nada— y el icono propio queda instalado y sin usar.
		expect(claves.get('Icon')).toBe(configuracion.productName);
	});

	test('y lo pide sin ruta, para que lo resuelva el tema', () => {
		expect(claves.get('Icon')).not.toContain('/');
	});

	test('el escritorio puede unirla con la ventana abierta', () => {
		// `StartupWMClass` tiene que ser el `identifier`, o el lanzador muestra
		// la aplicación como cerrada estando abierta.
		expect(claves.get('StartupWMClass')).toBe(configuracion.identifier);
	});

	test('no se muestra en el menú', () => {
		// Un lanzador en el menú es la única entrada que, al abrirla, abre lo
		// que ya estabas usando. Y aparecería en sus propios resultados.
		expect(claves.get('NoDisplay')).toBe('true');
	});

	test('está traducida al español', () => {
		// «Prism» es el nombre del programa y no se traduce; lo que el usuario
		// lee alrededor, sí. Una entrada en inglés entre las demás en español es
		// la parte que más se nota.
		for (const clave of ['GenericName', 'Comment', 'Keywords']) {
			expect(claves.has(`${clave}[es]`), `falta ${clave}[es]`).toBe(true);
			expect(claves.get(`${clave}[es]`)).not.toBe(claves.get(clave));
		}
		expect(claves.has('Name[es]')).toBe(false);
	});

	test('cae en una categoría del menú, y en una sola', () => {
		// Con dos categorías principales la aplicación aparecería dos veces.
		const categorias = (claves.get('Categories') ?? '').split(';').filter(Boolean);
		const principales = categorias.filter((una) =>
			[
				'AudioVideo', 'Audio', 'Video', 'Development', 'Education', 'Game',
				'Graphics', 'Network', 'Office', 'Science', 'Settings', 'System', 'Utility',
			].includes(una)
		);
		expect(principales).toHaveLength(1);
	});

	test('y el archivo entero es válido para el escritorio', () => {
		// `desktop-file-validate` es la referencia, y viene en
		// `desktop-file-utils`, que el paquete ya declara como dependencia. Si
		// no está en la máquina que corre las pruebas, no se comprueba: no es
		// motivo para que fallen.
		const validador = Bun.which('desktop-file-validate');
		if (!validador) {
			return;
		}
		const corrida = Bun.spawnSync([validador, new URL(ENTRADA, RAIZ).pathname]);
		const dijo =
			new TextDecoder().decode(corrida.stdout) + new TextDecoder().decode(corrida.stderr);
		// Los avisos que empiezan en «hint:» son sugerencias de estilo, no
		// errores; lo que no puede haber es un «error:».
		expect(dijo).not.toContain('error:');
		expect(corrida.exitCode).toBe(0);
	});
});
