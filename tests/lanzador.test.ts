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
import Lanzador from '@/vistas/Lanzador.vue';
import { contestar, emitir, laVentanaRecibio, loQueSePidio, olvidarTodo } from './dobles';

/** Más que la espera del lanzador, que es de 40 ms. */
const DESPUES_DE_LA_ESPERA = 80;

function fila(titulo: string, accion: string | null = null) {
	return {
		id: `${titulo}.desktop`,
		accion,
		titulo,
		subtitulo: null,
		icono: null,
		puntaje: 100,
	};
}

async function asentar(vueltas = 6) {
	for (let i = 0; i < vueltas; i++) {
		await nextTick();
	}
}

function dormir(ms: number) {
	return new Promise((listo) => setTimeout(listo, ms));
}

let vista: VueWrapper | null = null;

async function escribir(texto: string) {
	const campo = vista?.get('input');
	(campo?.element as HTMLInputElement).value = texto;
	await campo?.trigger('input');
}

async function teclear(key: string) {
	await vista?.get('input').trigger('keydown', { key });
	await asentar();
}

beforeEach(() => {
	olvidarTodo();
});

afterEach(() => {
	vista?.unmount();
	vista = null;
});

describe('buscar', () => {
	test('lo que se escribe se le pide al backend', async () => {
		contestar('buscar', async () => [fila('Firefox')]);
		vista = mount(Lanzador);

		await escribir('fire');
		await dormir(DESPUES_DE_LA_ESPERA);
		await asentar();

		expect(loQueSePidio.map((uno) => uno.comando)).toContain('buscar');
		expect(vista.text()).toContain('Firefox');
	});

	test('una palabra escrita rápido es una sola consulta', async () => {
		// La espera no es para no cargar al backend: es para no pintar una lista
		// por cada tecla.
		contestar('buscar', async () => []);
		vista = mount(Lanzador);

		for (const texto of ['f', 'fi', 'fir', 'fire']) {
			await escribir(texto);
		}
		await dormir(DESPUES_DE_LA_ESPERA);

		const consultas = loQueSePidio.filter((uno) => uno.comando === 'buscar');
		expect(consultas).toHaveLength(1);
		expect(consultas[0]?.args.consulta).toBe('fire');
	});

	test('con el campo vacío no se le pregunta nada al backend', async () => {
		// El lanzador abre con el campo vacío: preguntar ahí sería devolver el
		// catálogo entero para no mostrar nada.
		contestar('buscar', async () => [fila('Firefox')]);
		vista = mount(Lanzador);

		await escribir('  ');
		await dormir(DESPUES_DE_LA_ESPERA);
		await asentar();

		expect(loQueSePidio.filter((uno) => uno.comando === 'buscar')).toHaveLength(0);
	});

	test('una respuesta vieja no pisa a la nueva', async () => {
		// La carrera: la consulta que salió antes tarda más. Sin el número de
		// consulta se ve la lista de «fir» después de haber escrito «firefox».
		let soltarLaVieja = () => {};
		let cual = 0;
		contestar('buscar', async () => {
			cual++;
			if (cual === 1) {
				return new Promise((listo) => {
					soltarLaVieja = () => listo([fila('Lo viejo')]);
				});
			}
			return [fila('Lo nuevo')];
		});

		vista = mount(Lanzador);
		await escribir('fir');
		await dormir(DESPUES_DE_LA_ESPERA);
		await escribir('firefox');
		await dormir(DESPUES_DE_LA_ESPERA);
		await asentar();

		expect(vista.text()).toContain('Lo nuevo');

		// Y ahora contesta la primera, tarde.
		soltarLaVieja();
		await asentar();

		expect(vista.text()).toContain('Lo nuevo');
		expect(vista.text()).not.toContain('Lo viejo');
	});

	test('instalar algo con la ventana abierta rehace la búsqueda', async () => {
		let cuantas = 0;
		contestar('buscar', async () => {
			cuantas++;
			return cuantas === 1 ? [fila('Uno')] : [fila('Uno'), fila('Dos')];
		});

		vista = mount(Lanzador);
		await escribir('u');
		await dormir(DESPUES_DE_LA_ESPERA);
		await asentar();
		expect(vista.text()).not.toContain('Dos');

		await emitir('catalogo-cambiado');
		await asentar();

		expect(vista.text()).toContain('Dos');
	});
});

describe('el teclado', () => {
	async function conTres() {
		contestar('buscar', async () => [fila('Uno'), fila('Dos'), fila('Tres')]);
		vista = mount(Lanzador);
		await escribir('o');
		await dormir(DESPUES_DE_LA_ESPERA);
		await asentar();
	}

	function elegida() {
		return vista?.findAll('[role="option"]').findIndex(
			(una) => una.attributes('aria-selected') === 'true'
		);
	}

	test('la primera viene elegida', async () => {
		// Escribir y apretar Enter tiene que abrir lo primero sin tocar nada más.
		await conTres();
		expect(elegida()).toBe(0);
	});

	test('las flechas mueven la elección', async () => {
		await conTres();

		await teclear('ArrowDown');
		expect(elegida()).toBe(1);

		await teclear('ArrowUp');
		expect(elegida()).toBe(0);
	});

	test('y dan la vuelta en las dos puntas', async () => {
		// En una lista corta es más rápido que volver arriba a mano.
		await conTres();

		await teclear('ArrowUp');
		expect(elegida()).toBe(2);

		await teclear('ArrowDown');
		expect(elegida()).toBe(0);
	});

	test('Enter lanza la elegida y esconde la ventana', async () => {
		contestar('lanzar', async () => undefined);
		await conTres();

		await teclear('ArrowDown');
		await teclear('Enter');
		await asentar();

		const lanzada = loQueSePidio.find((uno) => uno.comando === 'lanzar');
		expect(lanzada?.args.id).toBe('Dos.desktop');
		expect(laVentanaRecibio).toContain('hide');
	});

	test('y la esconde también si el lanzamiento falla', async () => {
		// Dejarla abierta con la consulta puesta parece que no se apretó nada.
		contestar('lanzar', async () => {
			throw new Error('no se pudo');
		});
		await conTres();

		await teclear('Enter');
		await asentar();

		expect(laVentanaRecibio).toContain('hide');
	});

	test('Escape esconde y deja el campo limpio', async () => {
		// Esconder y no cerrar: la ventana se construye una vez, que es lo que
		// hace que abrir el lanzador sea instantáneo.
		await conTres();

		await teclear('Escape');
		await asentar();

		expect(laVentanaRecibio).toContain('hide');
		expect(laVentanaRecibio).not.toContain('close');
		expect((vista?.get('input').element as HTMLInputElement).value).toBe('');
	});

	test('con la lista vacía las flechas no hacen nada', async () => {
		contestar('buscar', async () => []);
		vista = mount(Lanzador);
		await escribir('zzz');
		await dormir(DESPUES_DE_LA_ESPERA);
		await asentar();

		await teclear('ArrowDown');
		await teclear('Enter');

		expect(loQueSePidio.filter((uno) => uno.comando === 'lanzar')).toHaveLength(0);
	});
});
