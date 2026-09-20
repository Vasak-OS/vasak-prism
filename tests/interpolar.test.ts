import { describe, expect, test } from 'bun:test';
import { claveSegunCantidad, interpolar } from '../src/tools/interpolar';

describe('interpolar', () => {
	test('reemplaza los marcadores por su valor', () => {
		expect(interpolar('Hola, {0}', 'Pato')).toBe('Hola, Pato');
		expect(interpolar('{0} de {1}', 3, 10)).toBe('3 de 10');
	});

	test('un valor con $& no altera el texto', () => {
		// Éste es el bug real: con `replace(marcador, valor)` el `$&` se expande
		// al texto encontrado, así que «Rock $& Roll» salía «Rock {0} Roll».
		expect(interpolar('Añadida: {0}', 'Rock $& Roll')).toBe('Añadida: Rock $& Roll');
	});

	test('un valor con $$ conserva los dos signos', () => {
		expect(interpolar('Precio: {0}', 'Cash $$ Money')).toBe('Precio: Cash $$ Money');
	});

	test("un valor con $' no se come el resto del texto", () => {
		// El peor de los tres: `$'` inserta lo que viene después de la
		// coincidencia, así que borraba el final de la frase.
		expect(interpolar("Tema: {0} (fin)", "Don't $' Stop")).toBe("Tema: Don't $' Stop (fin)");
	});

	test('un marcador repetido se reemplaza en todas sus apariciones', () => {
		expect(interpolar('{0} y {0}', 'uno')).toBe('uno y uno');
	});

	test('un marcador sin valor queda como está', () => {
		// Mejor que se vea el marcador que un «undefined» en la interfaz.
		expect(interpolar('{0} y {1}', 'uno')).toBe('uno y {1}');
	});

	test('una plantilla sin marcadores pasa intacta', () => {
		expect(interpolar('Sin marcadores', 'ignorado')).toBe('Sin marcadores');
	});
});

describe('claveSegunCantidad', () => {
	test('uno usa el singular y el resto el plural', () => {
		expect(claveSegunCantidad('inicio.pistas', 1)).toBe('inicio.pistasOne');
		expect(claveSegunCantidad('inicio.pistas', 2)).toBe('inicio.pistasOther');
	});

	test('cero usa el plural', () => {
		// En español y en inglés, cero va en plural: «0 pistas».
		expect(claveSegunCantidad('inicio.pistas', 0)).toBe('inicio.pistasOther');
	});
});
