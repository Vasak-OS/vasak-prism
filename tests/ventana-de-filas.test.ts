/**
 * Qué filas de la lista se dibujan.
 *
 * Es la cuenta que decide si un resultado se ve o no, y equivocarla no da
 * ningún error: la lista aparece corta, o con un hueco, o parpadeando al
 * desplazar. Por eso está aparte del componente y sin nada del DOM.
 */

import { describe, expect, test } from 'bun:test';
import {
	COLCHON,
	desplazamientoParaVer,
	ventanaDeFilas,
} from '../src/composables/useVentanaDeFilas';

const ALTO = 56;
/** Ocho filas a la vista, que es lo que muestra el lanzador. */
const VISIBLE = ALTO * 8;

describe('qué filas se dibujan', () => {
	test('sin resultados no se dibuja ninguna', () => {
		const ventana = ventanaDeFilas(0, ALTO, VISIBLE, 0);
		expect(ventana).toEqual({
			primera: 0,
			fin: 0,
			altoTotal: 0,
			desplazamientoDeLaPrimera: 0,
		});
	});

	test('cincuenta resultados no son cincuenta filas', () => {
		// El punto de todo esto: cincuenta filas en el DOM son cincuenta iconos
		// pedidos al tema, en cada tecla, para mostrar ocho.
		const ventana = ventanaDeFilas(50, ALTO, VISIBLE, 0);

		expect(ventana.primera).toBe(0);
		expect(ventana.fin).toBeLessThan(50);
		expect(ventana.fin).toBe(8 + COLCHON * 2);
	});

	test('el alto total es el de todas, para que la barra valga', () => {
		// Si fuera el de las dibujadas, la barra de desplazamiento diría que la
		// lista termina donde termina el colchón.
		expect(ventanaDeFilas(50, ALTO, VISIBLE, 0).altoTotal).toBe(50 * ALTO);
	});

	test('al desplazar, la ventana acompaña', () => {
		const ventana = ventanaDeFilas(50, ALTO, VISIBLE, ALTO * 20);

		expect(ventana.primera).toBe(20 - COLCHON);
		expect(ventana.desplazamientoDeLaPrimera).toBe((20 - COLCHON) * ALTO);
	});

	test('al final de la lista no se piden filas que no existen', () => {
		// Un `slice` más allá del final no falla, pero el hueco de abajo quedaría
		// mal medido y la lista daría un salto.
		const ventana = ventanaDeFilas(10, ALTO, VISIBLE, ALTO * 9);
		expect(ventana.fin).toBe(10);
	});

	test('un desplazamiento negativo no da una primera fila negativa', () => {
		// El rebote de algunos navegadores manda `scrollTop` negativo, y eso
		// dejaba la lista vacía justo al llegar arriba de todo.
		const ventana = ventanaDeFilas(50, ALTO, VISIBLE, -120);
		expect(ventana.primera).toBe(0);
		expect(ventana.fin).toBeGreaterThan(0);
	});

	test('sin alto medido todavía se dibuja al menos una fila', () => {
		// En el primer dibujo el contenedor no tiene alto: con cero filas
		// visibles la lista abriría vacía y sólo aparecería al mover el ratón.
		const ventana = ventanaDeFilas(50, ALTO, 0, 0);
		expect(ventana.fin).toBeGreaterThan(0);
	});
});

describe('que la fila elegida se vea', () => {
	test('si ya se ve, no se mueve nada', () => {
		expect(desplazamientoParaVer(2, ALTO, VISIBLE, 0)).toBe(0);
	});

	test('bajando, la lista sigue a la elegida', () => {
		// La fila 8 empieza donde termina lo visible: hay que correr una fila.
		expect(desplazamientoParaVer(8, ALTO, VISIBLE, 0)).toBe(ALTO * 9 - VISIBLE);
	});

	test('subiendo también', () => {
		expect(desplazamientoParaVer(3, ALTO, VISIBLE, ALTO * 10)).toBe(ALTO * 3);
	});

	test('dar la vuelta a la primera lleva la lista arriba de todo', () => {
		// Con las flechas la selección da la vuelta, y si la lista no la sigue la
		// elegida queda fuera de la pantalla.
		expect(desplazamientoParaVer(0, ALTO, VISIBLE, ALTO * 40)).toBe(0);
	});
});
