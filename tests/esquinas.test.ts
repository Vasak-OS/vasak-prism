/**
 * Las esquinas redondeadas salen del radio que la persona configuró.
 *
 * Todo el escritorio dibuja sus esquinas con el mismo radio: el que se elige en
 * Configuración, que el gestor de configuración escribe sobre el elemento raíz
 * como `--corner-radius`. `main.css` lo convierte en tres tokens —la tarjeta, el
 * detalle chico y la ventana— y de ahí salen las tres utilidades que se pueden
 * escribir: `rounded-corner`, `rounded-corner-sm` y `rounded-corner-window`.
 *
 * Lo que se comprueba es que las plantillas no escriban ninguna otra. Dos formas
 * de salirse, y ninguna falla al compilar:
 *
 *  1. Un nombre que no existe —`rounded-window` en lugar de
 *     `rounded-corner-window`—. Tailwind no emite ninguna regla para una clase
 *     que no corresponde a ningún token y no avisa: el elemento queda con las
 *     esquinas cuadradas. Es lo que le pasó al panel del lanzador, y en una
 *     ventana sin decoración del compositor parece una decisión de diseño.
 *  2. Un radio fijo de Tailwind —`rounded-lg`, `rounded-md`—. Ése sí se dibuja,
 *     pero con un número escrito en la hoja: un escritorio configurado con las
 *     esquinas rectas o muy redondeadas deja de verse coherente justo en el
 *     elemento que lo usa.
 */

import { describe, expect, test } from 'bun:test';
import { readdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';

const RAIZ = join(import.meta.dir, '..', 'src');

/**
 * `rounded-full` se deja pasar: es un círculo, no una esquina, y no tiene nada
 * que ver con el radio configurado. Lo redondo de una foto de perfil o de un
 * punto de estado no sigue a la preferencia de esquinas ni tendría que hacerlo.
 */
const REDONDO = 'rounded-full';

function plantillas(directorio: string): string[] {
	const salida: string[] = [];
	for (const entrada of readdirSync(directorio, { withFileTypes: true })) {
		const ruta = join(directorio, entrada.name);
		if (entrada.isDirectory()) salida.push(...plantillas(ruta));
		else if (entrada.name.endsWith('.vue')) salida.push(ruta);
	}
	return salida;
}

/** Las utilidades que `main.css` declara, leídas de la hoja y no escritas acá. */
function utilidadesDeclaradas(): Set<string> {
	const hoja = readFileSync(join(RAIZ, 'assets', 'main.css'), 'utf8');
	const nombres = new Set<string>();
	for (const [, token] of hoja.matchAll(/--radius-([a-z0-9-]+):/g)) {
		nombres.add(`rounded-${token}`);
	}
	return nombres;
}

/** Cada `rounded-…` de las plantillas, con el archivo donde está escrito. */
function esquinasEscritas(): { clase: string; archivo: string }[] {
	const encontradas: { clase: string; archivo: string }[] = [];
	for (const archivo of plantillas(RAIZ)) {
		const texto = readFileSync(archivo, 'utf8');
		// Sólo lo que está dentro de un atributo de clase: un `rounded-` nombrado
		// en un comentario es prosa y no dibuja nada.
		for (const [, valor] of texto.matchAll(/\bclass=["']([^"']*)["']/g)) {
			for (const [, clase] of valor.matchAll(/\b(rounded-[a-z0-9-]+)\b/g)) {
				encontradas.push({ clase, archivo: archivo.slice(RAIZ.length + 1) });
			}
		}
	}
	return encontradas;
}

describe('las esquinas de las plantillas', () => {
	test('los tres tokens del radio siguen declarados en la hoja', () => {
		// Si alguien renombra un token, lo de abajo pasaría por vacío en lugar de
		// fallar: no habría ninguna utilidad válida contra la cual comparar.
		const declaradas = utilidadesDeclaradas();

		expect([...declaradas].sort()).toEqual([
			'rounded-corner',
			'rounded-corner-sm',
			'rounded-corner-window',
		]);
	});

	test('todas salen del radio configurado', () => {
		const declaradas = utilidadesDeclaradas();

		const ajenas = esquinasEscritas().filter(
			({ clase }) => clase !== REDONDO && !declaradas.has(clase)
		);

		expect(
			ajenas.map(({ clase, archivo }) => `${archivo}: ${clase}`),
			'una esquina que no sale de un token del escritorio: o no se dibuja, ' +
				'o se dibuja con un número fijo que no sigue a la configuración'
		).toEqual([]);
	});

	test('y el panel del lanzador lleva la de la ventana', () => {
		// La regresión concreta: decía `rounded-window`, que no existe, así que el
		// panel abría con las esquinas cuadradas.
		const lanzador = readFileSync(join(RAIZ, 'vistas', 'Lanzador.vue'), 'utf8');

		expect(lanzador).toContain('rounded-corner-window');
	});
});
