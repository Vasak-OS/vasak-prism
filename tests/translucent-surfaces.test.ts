import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * El lanzador es una superficie del escritorio: translúcido, y sin
 * `backdrop-blur`.
 *
 * Con 0.15.0 el panel quedó en `ui-float`, que es opaco, y tapaba el
 * desenfoque que pone Wayfire detrás de la superficie de capa (corrección del
 * usuario del 02/10/2026, vue-libvasak `docs/once-ui.md` §13). La raíz del
 * panel lleva `bg-ui-shell` —el fondo de la ventana al 85 %, de vue-libvasak
 * 2.3.0— o un fondo con opacidad explícita (`bg-x/NN`), y nunca uno opaco. Es
 * la misma prueba que `vasak-desktop/tests/translucent-surfaces.test.ts`.
 */
const ROOT = join(import.meta.dir, '..');
const read = (file: string) => readFileSync(join(ROOT, file), 'utf8');

/** Saca los comentarios HTML cortando por sus delimitadores. */
function stripHtmlComments(text: string): string {
	let out = '';
	let index = 0;
	while (index < text.length) {
		const start = text.indexOf('<!--', index);
		if (start === -1) return out + text.slice(index);
		out += text.slice(index, start);
		const end = text.indexOf('-->', start + 4);
		if (end === -1) return out;
		index = end + 3;
	}
	return out;
}

const template = (file: string) => {
	const text = read(file);
	return stripHtmlComments(text.slice(text.indexOf('<template>'), text.lastIndexOf('</template>')));
};

/** Los fondos que nombra un trozo de plantilla (`bg-…`), sin variantes de estado. */
function backgroundsOf(classes: string): string[] {
	return [...classes.matchAll(/(?<![\w:/-])bg-([a-z][\w-]*(?:\/\d+)?)(?![\w/-])/g)].map((match) => match[1] as string);
}

/** Un fondo deja ver lo de atrás si es `ui-shell`, `transparent` o lleva `/NN` < 100. */
function isTranslucent(background: string): boolean {
	if (background === 'ui-shell' || background === 'transparent') return true;
	const alpha = background.match(/\/(\d+)$/)?.[1];
	return alpha !== undefined && Number(alpha) < 100;
}

/** Lo que mira la prueba, para poder probar la prueba. */
function surfaceProblems(classes: string[]): string[] {
	const problems: string[] = [];
	for (const group of classes) {
		const backgrounds = backgroundsOf(group);
		if (backgrounds.length === 0) problems.push(`sin fondo: «${group}»`);
		for (const background of backgrounds) {
			if (!isTranslucent(background)) problems.push(`opaco: bg-${background}`);
		}
		if (/backdrop-blur/.test(group)) problems.push('con backdrop-blur');
	}
	return problems;
}

/** Las clases de cada elemento de la plantilla, en orden. */
function classAttributes(markup: string): string[] {
	return [...markup.matchAll(/\sclass="([^"]*)"/g)].map((match) => match[1] as string);
}

describe('el lanzador deja ver el desenfoque de Wayfire', () => {
	test('la raíz del panel es translúcida y sin backdrop-blur', () => {
		const panel = classAttributes(template('src/views/Launcher.vue')).find((classes) => classes.includes('rounded-corner-window'));
		expect(panel, 'no se encontró el panel en Launcher.vue').toBeDefined();
		expect(surfaceProblems([panel as string])).toEqual([]);
	});

	test('lo que lo envuelve no le pone un fondo opaco detrás', () => {
		// El contenedor de pantalla completa que centra el panel no lleva fondo:
		// uno opaco ahí taparía el escritorio aunque el panel sea translúcido.
		const [outer] = classAttributes(template('src/views/Launcher.vue'));
		expect(outer).toContain('h-screen');
		for (const background of backgroundsOf(outer as string)) expect(isTranslucent(background)).toBe(true);
		expect(template('src/App.vue')).not.toMatch(/(?<![\w:/-])bg-(?!ui-shell|transparent)[a-z][\w-]*(?![\w/-])/);
	});

	test('ni el html ni el cuerpo pintan un fondo', () => {
		const css = read('src/assets/main.css');
		expect(css).not.toMatch(/(?:^|[\s,}])(?:html|body|:root|#app)\s*\{[^}]*(?<![\w-])background(?:-color)?\s*:\s*(?!transparent)/m);
		expect(read('index.html')).not.toMatch(/style="[^"]*background/);
	});

	test('ui-shell existe y es translúcida en la librería instalada', () => {
		const tokens = read('node_modules/@vasakgroup/vue-libvasak/dist/tokens.css');
		const shell = tokens.match(/--color-ui-shell:\s*([^;]+);/)?.[1] ?? '';
		expect(shell).toMatch(/^color-mix\(in srgb, var\(--use-ui-background\) (\d+)%, transparent\)$/);
		expect(Number(shell.match(/(\d+)%/)?.[1])).toBeLessThan(100);
	});
});

describe('la guardia de translucidez ve lo opaco cuando lo hay', () => {
	test('rechaza el fondo con el que quedó 0.15.0 y los de antes', () => {
		expect(surfaceProblems(['rounded-corner-window bg-ui-float shadow-surface-l'])).toEqual(['opaco: bg-ui-float']);
		expect(surfaceProblems(['bg-ui-bg border'])).toEqual(['opaco: bg-ui-bg']);
		expect(surfaceProblems(['bg-ui-bg/90 backdrop-blur-md'])).toEqual(['con backdrop-blur']);
		expect(surfaceProblems(['rounded-corner-window border'])).toHaveLength(1);
	});

	test('y deja pasar los translúcidos', () => {
		expect(surfaceProblems(['bg-ui-shell shadow-surface-l', 'bg-ui-bg/90 hover:bg-ui-hover'])).toEqual([]);
	});
});
