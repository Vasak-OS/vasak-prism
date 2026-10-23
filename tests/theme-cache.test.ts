import { describe, expect, test } from 'bun:test';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { applyTheme, saveTheme, THEME_KEY } from '../src/tools/theme-cache';

/** Un `Storage` de prueba: no toca el `localStorage` global. */
function testStore(initial: Record<string, string> = {}): Storage {
	const data = new Map(Object.entries(initial));
	return {
		getItem: (key: string) => data.get(key) ?? null,
		setItem: (key: string, value: string) => void data.set(key, value),
	} as unknown as Storage;
}

/** Un almacén inaccesible, como `localStorage` en modo privado o bloqueado. */
function throwingStore(): Storage {
	const fail = () => {
		throw new Error('SecurityError');
	};
	return { getItem: fail, setItem: fail } as unknown as Storage;
}

const newRoot = () => document.createElement('div');

describe('saveTheme', () => {
	test('guarda la clase oscura y sólo las variables `--*` del estilo en línea', () => {
		const root = newRoot();
		root.classList.add('dark');
		root.style.setProperty('--foo', 'red');
		root.style.setProperty('--ui-background', '#101010');
		root.style.color = 'blue'; // una propiedad común no es del esquema
		const store = testStore();

		saveTheme(root, store);

		expect(JSON.parse(store.getItem(THEME_KEY) as string)).toEqual({
			dark: true,
			vars: { '--foo': 'red', '--ui-background': '#101010' },
		});
	});

	test('sin la clase `.dark` guarda dark en falso', () => {
		const root = newRoot();
		root.style.setProperty('--foo', 'red');
		const store = testStore();

		saveTheme(root, store);

		expect(JSON.parse(store.getItem(THEME_KEY) as string).dark).toBe(false);
	});

	test('la clave es la que lee el script de index.html', () => {
		expect(THEME_KEY).toBe('vsk-theme');
	});

	test('un almacén inaccesible no rompe el arranque', () => {
		expect(() => saveTheme(newRoot(), throwingStore())).not.toThrow();
	});
});

describe('applyTheme', () => {
	test('reaplica la clase oscura y las variables recordadas', () => {
		const store = testStore({
			[THEME_KEY]: JSON.stringify({
				dark: true,
				vars: { '--foo': 'red', '--ui-background': '#101010' },
			}),
		});
		const root = newRoot();

		applyTheme(root, store);

		expect(root.classList.contains('dark')).toBe(true);
		expect(root.style.getPropertyValue('--foo')).toBe('red');
		expect(root.style.getPropertyValue('--ui-background')).toBe('#101010');
	});

	test('con dark en falso no agrega la clase `.dark`', () => {
		const store = testStore({
			[THEME_KEY]: JSON.stringify({ dark: false, vars: { '--foo': 'red' } }),
		});
		const root = newRoot();

		applyTheme(root, store);

		expect(root.classList.contains('dark')).toBe(false);
		expect(root.style.getPropertyValue('--foo')).toBe('red');
	});

	test('sin nada guardado no toca la raíz ni lanza', () => {
		const root = newRoot();

		expect(() => applyTheme(root, testStore())).not.toThrow();

		expect(root.classList.contains('dark')).toBe(false);
		expect(root.style.length).toBe(0);
	});

	test('un JSON inválido no lanza ni toca la raíz', () => {
		const root = newRoot();

		expect(() => applyTheme(root, testStore({ [THEME_KEY]: '{no es json' }))).not.toThrow();

		expect(root.style.length).toBe(0);
	});

	test('un almacén inaccesible no rompe el arranque', () => {
		expect(() => applyTheme(newRoot(), throwingStore())).not.toThrow();
	});
});

describe('ida y vuelta', () => {
	test('aplicar lo que dejó guardar reproduce el tema', () => {
		const origin = newRoot();
		origin.classList.add('dark');
		origin.style.setProperty('--foo', 'red');
		origin.style.setProperty('--ui-background', '#101010');
		const store = testStore();
		saveTheme(origin, store);

		const target = newRoot();
		applyTheme(target, store);

		expect(target.classList.contains('dark')).toBe(true);
		expect(target.style.getPropertyValue('--foo')).toBe('red');
		expect(target.style.getPropertyValue('--ui-background')).toBe('#101010');
	});
});

/**
 * El cableado: el módulo solo no evita el parpadeo, hace falta que el script
 * pre-boot de `index.html` lo reaplique antes de montar y que `App.vue` guarde
 * el tema una vez cargada la configuración.
 */
describe('cableado contra el parpadeo de tema', () => {
	const read = (path: string) =>
		readFileSync(fileURLToPath(new URL(path, import.meta.url)), 'utf8');

	test('index.html reaplica el tema guardado en el <head>, antes del bundle', () => {
		const html = read('../index.html');
		const scriptMatch = html.match(/<script>([\s\S]*?)<\/script>/);
		expect(scriptMatch).not.toBeNull();
		expect(html.indexOf('<script>')).toBeLessThan(html.indexOf('</head>'));
		expect(html.indexOf('<script>')).toBeLessThan(html.indexOf('type="module"'));

		const body = (scriptMatch as RegExpMatchArray)[1];
		expect(body).toContain(`"${THEME_KEY}"`);
		expect(body).toContain('classList.add("dark")');
		expect(body).toContain('setProperty');
	});

	test('el script de index.html, ejecutado, reproduce lo que guardó saveTheme', () => {
		const html = read('../index.html');
		const body = (html.match(/<script>([\s\S]*?)<\/script>/) as RegExpMatchArray)[1];
		const root = document.documentElement;
		const previous = localStorage.getItem(THEME_KEY);
		try {
			localStorage.setItem(
				THEME_KEY,
				JSON.stringify({ dark: true, vars: { '--tema-prueba': 'rebeccapurple' } })
			);

			new Function(body)();

			expect(root.classList.contains('dark')).toBe(true);
			expect(root.style.getPropertyValue('--tema-prueba')).toBe('rebeccapurple');
		} finally {
			root.classList.remove('dark');
			root.style.removeProperty('--tema-prueba');
			if (previous === null) localStorage.removeItem(THEME_KEY);
			else localStorage.setItem(THEME_KEY, previous);
		}
	});

	test('App.vue guarda el tema tras cada loadConfig, al montar y al cambiar', () => {
		const source = read('../src/App.vue');

		expect(source).toContain("from '@/tools/theme-cache'");
		expect(source.match(/loadConfig\(\)\.then\(\(\) => saveTheme\(\)\)/g)).toHaveLength(2);
	});
});
