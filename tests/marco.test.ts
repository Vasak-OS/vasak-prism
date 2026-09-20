/**
 * La ventana de la plantilla.
 *
 * Esto es el molde del que nacen las aplicaciones de VasakOS, así que lo que
 * esté mal acá se copia dieciséis veces. Ya pasó: cada repositorio nació con su
 * propio marco escrito a mano, y a los pocos meses dos ventanas del mismo
 * escritorio no se parecían.
 *
 * Lo que se comprueba es que el layout **no dibuje nada propio** —todo sale del
 * marco compartido— y que las ranuras lleguen a donde tienen que llegar. Una
 * ranura que no se reexpone no da ningún error: lo que se le ponga dentro
 * desaparece en silencio y la ventana abre vacía.
 */

import { afterEach, describe, expect, test } from 'bun:test';
import { AppBar, WindowControls, WindowFrame } from '@vasakgroup/vue-libvasak';
import { mount, type VueWrapper } from '@vue/test-utils';
import { h } from 'vue';
import WindowAppLayout from '@/layouts/WindowAppLayout.vue';
import { olvidarTodo, ponerLaConfiguracion } from './dobles';

let vista: VueWrapper | null = null;

function abrir(ranuras: Record<string, () => unknown> = {}) {
	vista = mount(WindowAppLayout, { slots: ranuras });
	return vista;
}

afterEach(() => {
	vista?.unmount();
	vista = null;
	olvidarTodo();
});

describe('el marco', () => {
	test('sale de la librería y no está copiado acá', () => {
		const ventana = abrir();

		expect(ventana.findComponent(WindowFrame).exists()).toBe(true);
	});

	test('y no hay un segundo borde dibujado a mano', () => {
		// `rounded-corner-window` es la esquina de la ventana y sale del marco.
		// Con dos, el borde y el fondo se dibujan dos veces y se ven los dos.
		const ventana = abrir();

		expect(ventana.findAll('.rounded-corner-window').length).toBe(1);
	});

	test('lleva los tres botones, con su nombre traducido', () => {
		// Dos cosas en una lectura, porque es una sola línea de la plantilla la
		// que puede romper las dos: qué botones hay, y cómo se llaman.
		//
		// Los botones: una ventana que no sea normal —un cuadro de diálogo, el
		// instalador— le pasa `:controls="[]"` al marco. La plantilla no es ese
		// caso: si naciera sin ellos, toda aplicación nueva arrancaría sin
		// botones y habría que acordarse de encenderlos.
		//
		// Los nombres: sin pasarlos salen en inglés, que son los valores por
		// omisión de la librería. Es el nombre accesible y no un texto a la
		// vista, así que lo único que lo dice es el lector de pantalla y nadie
		// lo ve al mirar la ventana.
		const ventana = abrir();

		expect(
			ventana
				.findComponent(WindowControls)
				.findAll('button')
				.map((boton) => boton.attributes('aria-label'))
		).toEqual(['ventana.minimizar', 'ventana.maximizar', 'ventana.cerrar']);
	});

	test('los nombres de los botones están traducidos', () => {
		// Sin esto salen en inglés —son los valores por omisión de la
		// librería—, y lo único que los dice es el lector de pantalla, así que
		// nadie lo ve al mirar la ventana.
		const ventana = abrir();
		const marco = ventana.findComponent(WindowFrame);

		expect(marco.props('closeLabel')).toBe('ventana.cerrar');
		expect(marco.props('closeLabel')).not.toBe('Close');
	});
});

describe('las ranuras', () => {
	test('lo que va dentro del layout es el contenido de la ventana', () => {
		// Sin la ranura por omisión, `<WindowAppLayout>…</WindowAppLayout>`
		// descarta en silencio todo lo que se le ponga dentro y la ventana abre
		// vacía. No hay ningún error: simplemente no aparece nada.
		const ventana = abrir({ default: () => h('p', { class: 'lo-mio' }, 'contenido') });

		expect(ventana.find('.lo-mio').exists()).toBe(true);
		expect(ventana.text()).not.toContain('Poné el contenido');
	});

	test('sin contenido queda el cartel que dice qué falta', () => {
		const ventana = abrir();

		expect(ventana.text()).toContain('Poné el contenido');
	});

	test('las cinco de la barra llegan a la barra', () => {
		// Cada una por separado: reexponer cuatro de cinco no da error, y la que
		// falta desaparece sin dejar rastro.
		for (const ranura of ['identidad', 'titulo', 'barra', 'centro', 'acciones']) {
			const ventana = mount(WindowAppLayout, {
				slots: { [ranura]: () => h('span', { class: 'marca' }, ranura) },
			});

			expect(ventana.findComponent(AppBar).find('.marca').text()).toBe(ranura);
			ventana.unmount();
		}
	});
});

describe('dónde va la barra', () => {
	test('la decide la configuración del escritorio y no el layout', async () => {
		// La plantilla no fija la posición: la lee el marco de
		// `window.barPosition`. Si el layout la fijara, las aplicaciones nuevas
		// nacerían ignorando la preferencia.
		ponerLaConfiguracion({ window: { barPosition: 'left' } });
		const ventana = abrir();
		await new Promise((listo) => setTimeout(listo, 0));
		await ventana.vm.$nextTick();

		expect(ventana.findComponent(WindowFrame).props('position')).toBe(null);
		expect(ventana.findComponent(AppBar).find('div').classes()).toContain('flex-col');
	});
});
