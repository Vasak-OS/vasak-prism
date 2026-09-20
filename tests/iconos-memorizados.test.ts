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

import { afterEach, beforeEach, describe, expect, test } from 'bun:test';
import { mount, type VueWrapper } from '@vue/test-utils';
import { olvidarLosIconosDelTema } from '@vasakgroup/vue-libvasak';
import { nextTick } from 'vue';
import ListaDeResultados from '@/componentes/ListaDeResultados.vue';
import type { Resultado } from '@/servicios/busqueda';
import { emitir, iconosPedidos, olvidarTodo } from './dobles';

/**
 * Deja pasar las vueltas de microtareas que tarda una resolución.
 *
 * `ThemeIcon` encadena el registro del oyente compartido y después la
 * resolución, y cada eslabón es un `await`: con menos vueltas la prueba mira
 * antes de que haya pasado nada y pasa por el motivo equivocado.
 */
async function asentar(vueltas = 8) {
	for (let i = 0; i < vueltas; i++) {
		await nextTick();
		await Promise.resolve();
	}
}

function resultado(titulo: string, icono: string | null): Resultado {
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

let montada: VueWrapper | null = null;

function montar(resultados: Resultado[]) {
	montada = mount(ListaDeResultados, { props: { resultados, elegida: 0 } });
	return montada;
}

beforeEach(() => {
	olvidarTodo();
	// El estado vive en el módulo de la librería y el módulo se comparte entre
	// pruebas: sin esto, una ve la memoria que dejó la anterior.
	olvidarLosIconosDelTema();
});

afterEach(() => {
	montada?.unmount();
	montada = null;
});

describe('los iconos de la lista', () => {
	test('cincuenta resultados no son cincuenta pedidos', async () => {
		// Cada uno con su icono distinto: lo único que puede bajar la cuenta es
		// que no se dibujen las filas que no se ven.
		const muchos = Array.from({ length: 50 }, (_, i) => resultado(`app-${i}`, `icono-${i}`));

		montar(muchos);
		await asentar();

		expect(iconosPedidos.length).toBeGreaterThan(0);
		expect(iconosPedidos.length).toBeLessThan(muchos.length);
	});

	test('diez filas de la misma aplicación son un pedido', async () => {
		// Pasa de verdad: una aplicación con acciones de escritorio aparece una
		// vez por acción, y todas llevan el icono de la aplicación.
		const repetidos = Array.from({ length: 10 }, (_, i) => resultado(`acción ${i}`, 'firefox'));

		montar(repetidos);
		await asentar();

		expect(iconosPedidos.filter((uno) => uno === 'firefox')).toHaveLength(1);
	});

	test('volver a montar la misma lista no vuelve a pedir nada', async () => {
		// Es el caso de cada tecla: la consulta cambia, la lista se rehace, y los
		// iconos son los mismos de hace un momento.
		const lista = [resultado('Firefox', 'firefox'), resultado('Terminal', 'terminal')];

		montar(lista);
		await asentar();
		const primeraVez = iconosPedidos.length;
		expect(primeraVez).toBe(2);

		montada?.unmount();
		montada = null;

		montar(lista);
		await asentar();
		expect(iconosPedidos.length).toBe(primeraVez);
	});

	test('una fila sin icono no pide nada', async () => {
		// Los resultados que no salen del disco —un cálculo, un emoji— vienen sin
		// icono, y ahí `ThemeIcon` no se monta.
		montar([resultado('2 + 2', null)]);
		await asentar();

		expect(iconosPedidos).toHaveLength(0);
	});

	test('cambiar el tema vuelve a resolver', async () => {
		// Lo contrario de lo anterior, y por eso va: una memoria que no se vacía
		// deja la ventana con los iconos del tema viejo hasta reabrirla.
		montar([resultado('Firefox', 'firefox')]);
		await asentar();
		expect(iconosPedidos).toHaveLength(1);

		await emitir('vicons:theme-changed');
		await asentar();

		expect(iconosPedidos).toHaveLength(2);
	});
});
