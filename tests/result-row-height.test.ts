/**
 * El alto de las filas de resultado no se mueve al pasarlas a `ListRow`.
 *
 * La lista es un desplazador virtual hecho a mano: calcula qué filas dibujar y
 * adónde desplazar con un alto fijo, sin medir ninguna. Si la fila midiera otra
 * cosa que ese número, nada fallaría: la barra de desplazamiento mentiría, la
 * flecha del teclado dejaría la elegida medio afuera y el colchón de abajo se
 * quedaría corto. Por eso el número se fija acá y se comprueba que llegue
 * entero a cada fila dibujada.
 *
 * Medido en el banco de Vite con Chrome sin pantalla, antes y después del
 * cambio, en claro y en oscuro y a 240, 360, 600 y 1200 de ancho: 56 px las
 * nueve filas, en las ocho combinaciones. happy-dom no maqueta, así que esta
 * prueba no puede medir; lo que sí puede es comprobar que el alto que manda es
 * el del estilo y que la fila de la librería no trae uno propio que le pelee.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { ListRow } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import { nextTick } from 'vue';
import ResultList, { RESULT_LIST_ID, RESULT_ROW_HEIGHT, resultOptionId } from '@/components/ResultList.vue';
import ResultRow from '@/components/ResultRow.vue';
import type { Resultado } from '@/servicios/busqueda';
import { olvidarTodo } from './dobles';

function result(title: string, subtitle: string | null = null): Resultado {
	return {
		id: `${title}.desktop`,
		accion: null,
		titulo: title,
		subtitulo: subtitle,
		subtituloDato: null,
		icono: null,
		puntaje: 1,
		origen: 'aplicacion',
	};
}

let mounted: VueWrapper | null = null;

afterEach(() => {
	mounted?.unmount();
	mounted = null;
	olvidarTodo();
});

describe('el alto de las filas de resultado', () => {
	test('es 56, el mismo que tenían antes de pasar a ListRow', () => {
		// Si alguien lo cambia, que sea a propósito: es el formato de la lista.
		expect(RESULT_ROW_HEIGHT).toBe(56);
	});

	test('llega entero a cada fila dibujada', async () => {
		mounted = mount(ResultList, {
			props: {
				results: [result('Firefox', 'Navegador'), result('Terminal'), result('Un título muy largo '.repeat(8))],
				selected: 0,
			},
		});
		await nextTick();

		const rows = mounted.findAll('[role="option"]');
		expect(rows).toHaveLength(3);
		for (const row of rows) {
			// La caja lleva el alto y la fila de la librería la llena.
			expect((row.element.parentElement as HTMLElement).style.height).toBe(`${RESULT_ROW_HEIGHT}px`);
			expect(row.classes()).toContain('h-full');
		}
	});

	test('y el hueco de la lista es filas por alto, sin medir nada', async () => {
		const results = Array.from({ length: 20 }, (_, i) => result(`app-${i}`));
		mounted = mount(ResultList, { props: { results, selected: 0 } });
		await nextTick();

		const spacer = mounted.get('[role="listbox"] > div').element as HTMLElement;
		expect(spacer.style.height).toBe(`${20 * RESULT_ROW_HEIGHT}px`);
	});

	test('la fila es la ListRow de la librería y no trae alto propio', () => {
		mounted = mount(ResultRow, {
			props: { result: result('Firefox', 'Navegador'), selected: false, height: RESULT_ROW_HEIGHT, id: 'row-0' },
		});

		const row = mounted.findComponent(ListRow);
		expect(row.exists()).toBe(true);
		expect((mounted.element as HTMLElement).style.height).toBe(`${RESULT_ROW_HEIGHT}px`);
		// Un `h-*` o `min-h-*` de la librería podría ganarle al `h-full` o
		// estirarla: la librería promete no imponerlo dentro de un desplazador
		// virtual, y esto lo sostiene. El único es el que pone el lanzador.
		const classes = row.classes();
		expect(classes.filter((name) => /^(?:min-|max-)?h-/.test(name))).toEqual(['h-full']);
		// Lo que entra adentro es menos que el alto: 32 del icono más el
		// relleno de 8 arriba y abajo. Con el relleno en `py-2`, la fila fija
		// no corta nada.
		expect(classes).toContain('py-2');
	});
});

describe('la fila como opción de la lista', () => {
	test('lleva el rol, la marca de elegida y un id para el campo', async () => {
		mounted = mount(ResultList, {
			props: { results: [result('Uno'), result('Dos')], selected: 1 },
		});
		await nextTick();

		expect(mounted.get('[role="listbox"]').attributes('id')).toBe(RESULT_LIST_ID);
		const rows = mounted.findAll('[role="option"]');
		expect(rows.map((row) => row.attributes('aria-selected'))).toEqual(['false', 'true']);
		expect(rows.map((row) => row.attributes('id'))).toEqual([resultOptionId(0), resultOptionId(1)]);
	});

	test('el clic elige y el puntero apunta, con el índice de la lista', async () => {
		mounted = mount(ResultList, {
			props: { results: [result('Uno'), result('Dos')], selected: 0 },
		});
		await nextTick();

		const second = mounted.findAll('[role="option"]')[1];
		// `mouseenter` no burbujea: lo oye la caja de la fila, que es donde
		// entra el puntero.
		second?.element.parentElement?.dispatchEvent(new MouseEvent('mouseenter'));
		await second?.trigger('click');

		expect(mounted.emitted('point')).toEqual([[1]]);
		expect(mounted.emitted('choose')).toEqual([[1]]);
	});

	test('apretar no le saca el foco al campo', async () => {
		mounted = mount(ResultRow, {
			props: { result: result('Uno'), selected: false, height: RESULT_ROW_HEIGHT, id: 'row-0' },
		});

		// Sobre la fila de adentro, que es donde se aprieta: llega a la caja
		// burbujeando.
		const event = new MouseEvent('mousedown', { bubbles: true, cancelable: true });
		mounted.get('[role="option"]').element.dispatchEvent(event);

		expect(event.defaultPrevented).toBe(true);
	});
});

describe('la elegida no se pierde al desplazar con la rueda', () => {
	let original: PropertyDescriptor | undefined;

	afterEach(() => {
		if (original) Object.defineProperty(HTMLElement.prototype, 'clientHeight', original);
		original = undefined;
	});

	test('sigue en el DOM, en su lugar, para que el campo la pueda anunciar', async () => {
		// Ocho filas a la vista: happy-dom no maqueta y sin esto la ventana sale
		// de un alto cero.
		original = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight');
		Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
			configurable: true,
			get: () => 8 * RESULT_ROW_HEIGHT,
		});
		const results = Array.from({ length: 50 }, (_, i) => result(`app-${i}`));
		mounted = mount(ResultList, { props: { results, selected: 0 } });
		await nextTick();

		const box = mounted.get('[role="listbox"]');
		(box.element as HTMLElement).scrollTop = 30 * RESULT_ROW_HEIGHT;
		await box.trigger('scroll');

		const chosen = mounted.find(`#${resultOptionId(0)}`);
		expect(chosen.exists()).toBe(true);
		expect(chosen.attributes('aria-selected')).toBe('true');
		expect((chosen.element.parentElement?.parentElement as HTMLElement).style.top).toBe('0px');
		// Y una sola vez: el resto de la ventana no la repite.
		expect(mounted.findAll('[aria-selected="true"]')).toHaveLength(1);
	});

	test('dentro de la ventana no se dibuja dos veces', async () => {
		mounted = mount(ResultList, { props: { results: [result('Uno'), result('Dos')], selected: 1 } });
		await nextTick();

		expect(mounted.findAll(`#${resultOptionId(1)}`)).toHaveLength(1);
	});
});
