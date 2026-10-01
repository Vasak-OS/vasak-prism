/**
 * Cuántas veces se le pide un icono al tema.
 *
 * Es la mitad medible de #15. El plan original quería guardar el icono ya
 * resuelto en el índice; lo que hace falta de verdad es no pedirlo dos veces, y
 * eso son dos cosas que se comprueban acá porque ninguna se ve mirando la
 * pantalla:
 *
 * - la lista dibuja sólo las filas visibles, así que cincuenta resultados no son
 *   cincuenta pedidos;
 * - `ThemeIcon` memoriza por nombre, así que diez filas de la misma aplicación
 *   son un pedido.
 *
 * Sin esta prueba las dos se pueden perder sin que nada se vea distinto: los
 * iconos siguen apareciendo igual, sólo que costando de más en cada tecla.
 */

import { afterEach, beforeEach, describe, expect, jest, test } from 'bun:test';
import { mount, type VueWrapper } from '@vue/test-utils';
import { olvidarLosIconosDelTema } from '@vasakgroup/vue-libvasak';
import { nextTick } from 'vue';
import ResultList, { RESULT_ROW_HEIGHT } from '@/components/ResultList.vue';
import type { Resultado } from '@/servicios/busqueda';
import { emitir, iconosPedidos, olvidarTodo, ponerEnElTema } from './dobles';

/**
 * Deja pasar las vueltas de microtareas que tarda una resolución.
 *
 * `ThemeIcon` encadena el registro del oyente compartido y después la
 * resolución, y cada eslabón es un `await`: con menos vueltas la prueba mira
 * antes de que haya pasado nada y pasa por el motivo equivocado.
 */
async function settle(rounds = 8) {
	for (let i = 0; i < rounds; i++) {
		await nextTick();
		await Promise.resolve();
	}
}

function result(titulo: string, icono: string | null): Resultado {
	return {
		id: `${titulo}.desktop`,
		accion: null,
		titulo,
		subtitulo: null,
		subtituloDato: null,
		icono,
		puntaje: 100,
		origen: 'aplicacion',
	};
}

/** Lo que mide una fila: el del componente, no una copia. */
const ROW_HEIGHT = RESULT_ROW_HEIGHT;
/** Cuántas entran en la ventana que finge esta prueba. */
const VISIBLE_ROWS = 8;
/** El colchón que dibuja el componente arriba y abajo. */
const CUSHION = 3;

let mounted: VueWrapper | null = null;
let originalHeight: PropertyDescriptor | undefined;

/**
 * Le da un alto a la caja.
 *
 * Sin esto `clientHeight` es cero —happy-dom no hace maquetado— y la ventana
 * visible sale de una cuenta que nadie eligió: la prueba pasaría por el número
 * equivocado y dejaría de pasar el día que cambie el colchón.
 */
function withViewportOf(rows: number) {
	originalHeight = Object.getOwnPropertyDescriptor(HTMLElement.prototype, 'clientHeight');
	Object.defineProperty(HTMLElement.prototype, 'clientHeight', {
		configurable: true,
		get: () => rows * ROW_HEIGHT,
	});
}

function mountList(results: Resultado[]) {
	mounted = mount(ResultList, { props: { results: results, selected: 0 } });
	return mounted;
}

beforeEach(() => {
	olvidarTodo();
	// El estado vive en el módulo de la librería y el módulo se comparte entre
	// pruebas: sin esto, una ve la memoria que dejó la anterior.
	olvidarLosIconosDelTema();
});

afterEach(() => {
	mounted?.unmount();
	mounted = null;

	if (originalHeight) {
		Object.defineProperty(HTMLElement.prototype, 'clientHeight', originalHeight);
		originalHeight = undefined;
	}
});

describe('los iconos de la lista', () => {
	test('cincuenta resultados piden catorce iconos', async () => {
		// Cada uno con su icono distinto: lo único que puede bajar la cuenta es
		// que no se dibujen las filas que no se ven. Catorce y no «menos de
		// cincuenta» porque el número es el que se quiere: las ocho que entran
		// más el colchón de tres de cada lado, y nada más.
		withViewportOf(VISIBLE_ROWS);
		const many = Array.from({ length: 50 }, (_, i) => result(`app-${i}`, `icono-${i}`));

		mountList(many);
		await settle();

		const esperados = VISIBLE_ROWS + CUSHION * 2;
		expect(iconosPedidos).toHaveLength(esperados);
		// Y son los de arriba de todo, que son los que se ven.
		expect(iconosPedidos.slice().sort()).toEqual(
			Array.from({ length: esperados }, (_, i) => `icono-${i}`).sort()
		);
	});

	test('diez filas de la misma aplicación son un pedido', async () => {
		// Pasa de verdad: una aplicación con acciones de escritorio aparece una
		// vez por acción, y todas llevan el icono de la aplicación.
		const repetidos = Array.from({ length: 10 }, (_, i) => result(`acción ${i}`, 'firefox'));

		mountList(repetidos);
		await settle();

		expect(iconosPedidos.filter((uno) => uno === 'firefox')).toHaveLength(1);
	});

	test('volver a montar la misma lista no vuelve a pedir nada', async () => {
		// Es el caso de cada tecla: la consulta cambia, la lista se rehace, y los
		// iconos son los mismos de hace un momento.
		const lista = [result('Firefox', 'firefox'), result('Terminal', 'terminal')];

		mountList(lista);
		await settle();
		const primeraVez = iconosPedidos.length;
		expect(primeraVez).toBe(2);

		mounted?.unmount();
		mounted = null;

		mountList(lista);
		await settle();
		expect(iconosPedidos).toHaveLength(primeraVez);
	});

	test('una fila sin icono no pide nada', async () => {
		// Los resultados que no salen del disco —un cálculo, un emoji— vienen sin
		// icono, y ahí `ThemeIcon` no se monta.
		mountList([result('2 + 2', null)]);
		await settle();

		expect(iconosPedidos).toHaveLength(0);
	});

	test('cambiar el tema vuelve a resolver, pero no en el acto', async () => {
		// Lo contrario de lo anterior, y por eso va: una memoria que no se vacía
		// deja la ventana con los iconos del tema viejo hasta reabrirla.
		//
		// Desde la 1.3.0 de la librería el pedido nuevo **no** sale enseguida:
		// la memoria se vacía y la recarga se agenda, para que el aviso del tema
		// de iconos y el de GTK no disparen dos barridos. Con una lista que se
		// redibuja en cada tecla eso es la diferencia entre un barrido y dos.
		// Antes esta prueba esperaba sólo microtareas y por eso contaba dos
		// pedidos; ahora hay que adelantar el reloj, con temporizadores falsos
		// para que no dependa de cuán cargada esté la máquina.
		// Y se mira **el dibujo**, no sólo la cuenta de pedidos: sin dos fuentes
		// distintas los dos pedidos devuelven lo mismo, así que una lista que se
		// rompiera al redibujar contaría igual dos y pasaría. Lo marcó la
		// revisión.
		jest.useFakeTimers();
		try {
			ponerEnElTema('firefox', 'data:image/svg+xml,zorro-claro');
			const lista = mountList([result('Firefox', 'firefox')]);
			await settle();
			expect(iconosPedidos).toHaveLength(1);
			expect(lista.get('img').attributes('src')).toBe('data:image/svg+xml,zorro-claro');

			ponerEnElTema('firefox', 'data:image/svg+xml,zorro-oscuro');
			await emitir('vicons:theme-changed');
			await settle();

			// Todavía no: la recarga está agendada.
			expect(iconosPedidos).toHaveLength(1);
			expect(lista.get('img').attributes('src')).toBe('data:image/svg+xml,zorro-claro');

			// El planificador `await`ea entre tandas, así que se avanza en pasos
			// con microtareas en el medio.
			for (let i = 0; i < 8; i++) {
				jest.advanceTimersByTime(40);
				await settle(2);
			}

			expect(iconosPedidos).toHaveLength(2);
			expect(lista.get('img').attributes('src')).toBe('data:image/svg+xml,zorro-oscuro');
		} finally {
			jest.useRealTimers();
		}
	});
});
