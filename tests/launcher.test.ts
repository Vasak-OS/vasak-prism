/**
 * La ventana del lanzador, montada.
 *
 * Lo que se comprueba acá no es el dibujo: es el teclado y las carreras. Un
 * lanzador se usa sin mirar —se escribe, se aprieta Enter— y las dos formas de
 * romperlo son que la flecha no lleve a donde se ve y que la lista que se ve no
 * sea la de lo que se escribió.
 */

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import { ListRow, SearchField } from '@vasakgroup/vue-libvasak';
import { RESULT_LIST_ID, resultOptionId } from '@/components/ResultList.vue';
import Launcher from '@/views/Launcher.vue';
import { contestar, emitir, laVentanaRecibio, loQueSePidio, olvidarTodo } from './dobles';

/** Más que la espera del lanzador, que es de 40 ms. */
const AFTER_DELAY = 80;

/** Si se le pidió al backend que esconda la ventana. */
function wasHidden() {
	return loQueSePidio.some((call) => call.comando === 'esconder');
}

function row(titulo: string, accion: string | null = null) {
	return {
		id: `${titulo}.desktop`,
		accion,
		titulo,
		subtitulo: null,
		subtituloDato: null,
		icono: null,
		puntaje: 100,
		origen: 'aplicacion' as const,
	};
}

/** Una fila de cualquier otro proveedor. */
function fromProvider(
	origen:
		| 'calculo'
		| 'emoji'
		| 'web'
		| 'comando'
		| 'reciente'
		| 'completar'
		| 'configuracion'
		| 'ventana'
		| 'archivo',
	id: string
) {
	return {
		id,
		accion: null,
		titulo: id,
		subtitulo: null,
		subtituloDato: null,
		icono: null,
		puntaje: 1000,
		origen,
	};
}

/** Una fila de cálculo: lo que se copia va en `id`. */
function calculation(value: string) {
	return {
		id: value,
		accion: null,
		titulo: value,
		subtitulo: 'lanzador.copiar',
		subtituloDato: null,
		icono: 'accessories-calculator',
		puntaje: 1000,
		origen: 'calculo' as const,
	};
}

async function settle(rounds = 6) {
	for (let i = 0; i < rounds; i++) {
		await nextTick();
	}
}

function sleep(ms: number) {
	return new Promise((done) => setTimeout(done, ms));
}

let view: VueWrapper | null = null;

async function type(texto: string) {
	const field = view?.get('input');
	(field?.element as HTMLInputElement).value = texto;
	await field?.trigger('input');
}

async function press(key: string) {
	await view?.get('input').trigger('keydown', { key });
	await settle();
}

beforeEach(() => {
	olvidarTodo();
});

afterEach(() => {
	view?.unmount();
	view = null;
});

describe('buscar', () => {
	test('lo que se escribe se le pide al backend', async () => {
		contestar('buscar', async () => [row('Firefox')]);
		view = mount(Launcher);

		await type('fire');
		await sleep(AFTER_DELAY);
		await settle();

		expect(loQueSePidio.map((call) => call.comando)).toContain('buscar');
		expect(view.text()).toContain('Firefox');
	});

	test('una palabra escrita rápido es una sola consulta', async () => {
		// La espera no es para no cargar al backend: es para no pintar una lista
		// por cada tecla.
		contestar('buscar', async () => []);
		view = mount(Launcher);

		for (const texto of ['f', 'fi', 'fir', 'fire']) {
			await type(texto);
		}
		await sleep(AFTER_DELAY);

		const queries = loQueSePidio.filter((call) => call.comando === 'buscar');
		expect(queries).toHaveLength(1);
		expect(queries[0]?.args.consulta).toBe('fire');
	});

	test('con el campo vacío no se le pregunta nada al backend', async () => {
		// El lanzador abre con el campo vacío: preguntar ahí sería devolver el
		// catálogo entero para no mostrar nada.
		contestar('buscar', async () => [row('Firefox')]);
		view = mount(Launcher);

		await type('  ');
		await sleep(AFTER_DELAY);
		await settle();

		expect(loQueSePidio.filter((call) => call.comando === 'buscar')).toHaveLength(0);
	});

	test('una respuesta vieja no pisa a la nueva', async () => {
		// La carrera: la consulta que salió antes tarda más. Sin el número de
		// consulta se ve la lista de «fir» después de haber escrito «firefox».
		let soltarLaVieja = () => {};
		let cual = 0;
		contestar('buscar', async () => {
			cual++;
			if (cual === 1) {
				return new Promise((done) => {
					soltarLaVieja = () => done([row('Lo viejo')]);
				});
			}
			return [row('Lo nuevo')];
		});

		view = mount(Launcher);
		await type('fir');
		await sleep(AFTER_DELAY);
		await type('firefox');
		await sleep(AFTER_DELAY);
		await settle();

		expect(view.text()).toContain('Lo nuevo');

		// Y ahora contesta la primera, tarde.
		soltarLaVieja();
		await settle();

		expect(view.text()).toContain('Lo nuevo');
		expect(view.text()).not.toContain('Lo viejo');
	});

	test('instalar algo con la ventana abierta rehace la búsqueda', async () => {
		let cuantas = 0;
		contestar('buscar', async () => {
			cuantas++;
			return cuantas === 1 ? [row('Uno')] : [row('Uno'), row('Dos')];
		});

		view = mount(Launcher);
		await type('u');
		await sleep(AFTER_DELAY);
		await settle();
		expect(view.text()).not.toContain('Dos');

		await emitir('catalogo-cambiado');
		await settle();

		expect(view.text()).toContain('Dos');
	});
});

describe('cuando la ventana vuelve a aparecer', () => {
	test('queda como recién abierta', async () => {
		// La ventana se construye al levantar la sesión y vive escondida: el
		// montaje pasa una vez y la apertura, cientos. Sin limpiar al aparecer,
		// el lanzador se abre con lo que se escribió la vez anterior.
		contestar('buscar', async () => [row('Firefox')]);
		view = mount(Launcher);

		await type('fire');
		await sleep(AFTER_DELAY);
		await settle();
		expect(view.text()).toContain('Firefox');

		await emitir('prism:mostrada');
		await settle();

		expect((view.get('input').element as HTMLInputElement).value).toBe('');
		expect(view.text()).not.toContain('Firefox');
	});

	test('y una respuesta de antes de cerrar no aparece después', async () => {
		// La consulta que quedó en vuelo al esconder la ventana contesta cuando
		// ya se reabrió: sin descartarla, aparecen los resultados de la búsqueda
		// anterior sobre un campo vacío.
		let contestarLaDeAntes = () => {};
		contestar('buscar', async () => {
			return new Promise((done) => {
				contestarLaDeAntes = () => done([row('Lo de antes')]);
			});
		});

		view = mount(Launcher);
		await type('fire');
		await sleep(AFTER_DELAY);

		await emitir('prism:mostrada');
		await settle();
		contestarLaDeAntes();
		await settle();

		expect(view.text()).not.toContain('Lo de antes');
	});
});

describe('el teclado', () => {
	async function conTres() {
		contestar('buscar', async () => [row('Uno'), row('Dos'), row('Tres')]);
		view = mount(Launcher);
		await type('o');
		await sleep(AFTER_DELAY);
		await settle();
	}

	function selectedIndex() {
		return view?.findAll('[role="option"]').findIndex(
			(option) => option.attributes('aria-selected') === 'true'
		);
	}

	test('la primera viene elegida', async () => {
		// Escribir y apretar Enter tiene que abrir lo primero sin tocar nada más.
		await conTres();
		expect(selectedIndex()).toBe(0);
	});

	test('las flechas mueven la elección', async () => {
		await conTres();

		await press('ArrowDown');
		expect(selectedIndex()).toBe(1);

		await press('ArrowUp');
		expect(selectedIndex()).toBe(0);
	});

	test('y dan la vuelta en las dos puntas', async () => {
		// En una lista corta es más rápido que volver arriba a mano.
		await conTres();

		await press('ArrowUp');
		expect(selectedIndex()).toBe(2);

		await press('ArrowDown');
		expect(selectedIndex()).toBe(0);
	});

	test('Enter lanza la elegida y esconde la ventana', async () => {
		contestar('lanzar', async () => undefined);
		await conTres();

		await press('ArrowDown');
		await press('Enter');
		await settle();

		const launched = loQueSePidio.find((call) => call.comando === 'lanzar');
		expect(launched?.args.id).toBe('Dos.desktop');
		expect(wasHidden()).toBe(true);
	});

	test('y la esconde también si el lanzamiento falla', async () => {
		// Dejarla abierta con la consulta puesta parece que no se apretó nada.
		contestar('lanzar', async () => {
			throw new Error('no se pudo');
		});
		await conTres();

		await press('Enter');
		await settle();

		expect(wasHidden()).toBe(true);
	});

	test('Escape esconde y deja el campo limpio', async () => {
		// Esconder y no cerrar: la ventana se construye una vez, que es lo que
		// hace que abrir el lanzador sea instantáneo.
		await conTres();

		await press('Escape');
		await settle();

		expect(wasHidden()).toBe(true);
		expect((view?.get('input').element as HTMLInputElement).value).toBe('');
	});

	test('esconder pasa por el backend y no por la ventana de Tauri', async () => {
		// Lo que se ve es la superficie de capa a la que se mudó el WebView; el
		// armazón de Tauri está escondido y vacío desde que arrancó, así que
		// esconderlo a él no hace nada visible.
		await conTres();

		await press('Escape');
		await settle();

		expect(laVentanaRecibio).not.toContain('hide');
	});

	test('una cuenta se copia y no se lanza', async () => {
		// Apretar Enter sobre «4» no puede intentar abrir un programa llamado
		// «4»: la fila dice de dónde salió y eso decide qué pasa.
		contestar('buscar', async () => [calculation('4')]);
		contestar('copiar', async () => undefined);
		view = mount(Launcher);

		await type('2+2');
		await sleep(AFTER_DELAY);
		await settle();
		await press('Enter');
		await settle();

		const copied = loQueSePidio.find((call) => call.comando === 'copiar');
		expect(copied?.args.texto).toBe('4');
		expect(loQueSePidio.filter((call) => call.comando === 'lanzar')).toHaveLength(0);
		expect(wasHidden()).toBe(true);
	});

	test('cada proveedor hace lo suyo al apretar Enter', async () => {
		// La fila dice de dónde salió y eso decide qué pasa: un emoji se copia,
		// una dirección se abre y un comando se ejecuta. Adivinar mirando la
		// forma del resultado es como se termina con dos lugares que tienen que
		// estar de acuerdo.
		const scenarios = [
			{ row: fromProvider('emoji', '🔥'), comando: 'copiar', key: 'texto', expected: '🔥' },
			{
				row: fromProvider('web', 'https://duckduckgo.com/?q=gatos'),
				comando: 'abrir',
				key: 'destino',
				expected: 'https://duckduckgo.com/?q=gatos',
			},
			{
				row: fromProvider('reciente', '/home/pato/notas.md'),
				comando: 'abrir',
				key: 'destino',
				expected: '/home/pato/notas.md',
			},
			{
				row: fromProvider('comando', 'systemctl --user status'),
				comando: 'ejecutar',
				key: 'comandoEscrito',
				expected: 'systemctl --user status',
			},
			{
				row: fromProvider('configuracion', 'network-wifi'),
				comando: 'abrir_configuracion',
				key: 'seccion',
				expected: 'network-wifi',
			},
			{
				row: fromProvider('ventana', '12'),
				comando: 'presentar_ventana',
				key: 'id',
				expected: '12',
			},
			{
				row: fromProvider('archivo', '/home/pato/notas.md'),
				comando: 'abrir',
				key: 'destino',
				expected: '/home/pato/notas.md',
			},
		];

		for (const scenario of scenarios) {
			olvidarTodo();
			contestar('buscar', async () => [scenario.row]);
			contestar(scenario.comando, async () => undefined);
			view?.unmount();
			view = mount(Launcher);

			await type('x');
			await sleep(AFTER_DELAY);
			await settle();
			await press('Enter');
			await settle();

			const request = loQueSePidio.find((call) => call.comando === scenario.comando);
			expect(request?.args[scenario.key], `${scenario.row.origen} tenía que ir a ${scenario.comando}`).toBe(
				scenario.expected
			);
			expect(loQueSePidio.filter((call) => call.comando === 'lanzar')).toHaveLength(0);
		}
	});

	test('una fila de completar escribe en el campo y no cierra', async () => {
		// Un bang a medias: ofrecerle a alguien que siga escribiendo es más útil
		// que no ofrecerle nada, y cerrar la ventana ahí sería lo contrario de lo
		// que pidió.
		contestar('buscar', async () => [fromProvider('completar', '!w ')]);
		view = mount(Launcher);

		await type('?!w');
		await sleep(AFTER_DELAY);
		await settle();
		await press('Enter');
		await settle();

		expect((view.get('input').element as HTMLInputElement).value).toBe('!w ');
		expect(wasHidden()).toBe(false);
	});

	test('con la lista vacía las flechas no hacen nada', async () => {
		contestar('buscar', async () => []);
		view = mount(Launcher);
		await type('zzz');
		await sleep(AFTER_DELAY);
		await settle();

		await press('ArrowDown');
		await press('Enter');

		expect(loQueSePidio.filter((call) => call.comando === 'lanzar')).toHaveLength(0);
	});
});

describe('la ayuda de los prefijos', () => {
	test('se ve con el campo vacío', async () => {
		// Es el único momento en que hay lugar para decir que los prefijos
		// existen, y una sintaxis que no se ve es una sintaxis que no se usa: es
		// lo que le pasó a la búsqueda vieja, que no tenía ni atajo.
		const launcher = mount(Launcher);
		await nextTick();

		expect(launcher.text()).toContain('lanzador.prefijos');
		launcher.unmount();
	});

	test('y se va apenas se escribe', async () => {
		// Con algo escrito el lugar es de los resultados.
		contestar('buscar', async () => []);
		const launcher = mount(Launcher);
		await launcher.find('input').setValue('firefox');
		await new Promise((done) => setTimeout(done, AFTER_DELAY));
		await nextTick();

		expect(launcher.text()).not.toContain('lanzador.prefijos');
		launcher.unmount();
	});
});

describe('las piezas salen de la librería', () => {
	test('el campo es el SearchField grande y sin canto', async () => {
		view = mount(Launcher);
		await nextTick();

		const search = view.findComponent(SearchField);
		expect(search.exists()).toBe(true);
		expect(search.props('size')).toBe('lg');
		expect(search.props('bare')).toBe(true);
		// Sin la cruz: Escape ya vacía y esconde, y una cruz sería un control
		// que la pantalla no tenía.
		expect(search.props('clearable')).toBe(false);
	});

	test('y anuncia la lista que maneja y la opción marcada', async () => {
		// Es lo que deja que un lector de pantalla diga por cuál resultado se
		// pasa con las flechas sin sacar el foco del campo.
		contestar('buscar', async () => [row('Firefox'), row('Files')]);
		view = mount(Launcher);
		await type('fi');
		await sleep(AFTER_DELAY);
		await settle();

		const input = view.get('input');
		expect(input.attributes('role')).toBe('combobox');
		expect(input.attributes('aria-controls')).toBe(RESULT_LIST_ID);
		expect(input.attributes('aria-expanded')).toBe('true');
		expect(input.attributes('aria-activedescendant')).toBe(resultOptionId(0));

		await press('ArrowDown');
		expect(view.get('input').attributes('aria-activedescendant')).toBe(resultOptionId(1));
		expect(view.find(`#${resultOptionId(1)}`).attributes('aria-selected')).toBe('true');
	});

	test('sin resultados no dice que maneje una lista', async () => {
		view = mount(Launcher);
		await nextTick();

		const input = view.get('input');
		expect(input.attributes('role')).toBeUndefined();
		expect(input.attributes('aria-controls')).toBeUndefined();
		expect(input.attributes('aria-activedescendant')).toBeUndefined();
	});

	test('las filas son ListRow', async () => {
		contestar('buscar', async () => [row('Firefox')]);
		view = mount(Launcher);
		await type('fire');
		await sleep(AFTER_DELAY);
		await settle();

		expect(view.findAllComponents(ListRow)).toHaveLength(1);
	});

	test('el panel es la superficie translúcida del escritorio, con la esquina de la ventana', async () => {
		// La regresión de antes: decía `rounded-window`, que no existe, y el
		// panel abría con las esquinas cuadradas. Y va en `ui-shell`, no en
		// `ui-float`: opaco tapaba el desenfoque que pone Wayfire detrás
		// (vue-libvasak `docs/once-ui.md` §13).
		view = mount(Launcher);
		await nextTick();

		const panel = view.get('.rounded-corner-window');
		expect(panel.classes()).toEqual(expect.arrayContaining(['bg-ui-shell', 'shadow-surface-l', 'border-ui-line']));
		expect(panel.classes().some((name) => /^bg-ui-(?:bg|float)/.test(name))).toBe(false);
		expect(panel.classes().some((name) => name.includes('backdrop-blur'))).toBe(false);
	});
});
