/**
 * Interpolación de textos traducidos.
 *
 * El `t()` del plugin de i18n toma **un solo argumento** y no interpola: la
 * convención del proyecto es el marcador `{0}` en el `.yml` y el reemplazo a
 * mano. Esto lo encapsula, porque hacerlo a mano tiene una trampa que ya nos
 * costó un bug real.
 */

/**
 * Reemplaza `{0}`, `{1}`, … por los valores dados.
 *
 * **Con función de reemplazo, no con el valor directo.**
 * `String.prototype.replace` interpreta `$&`, `$$`, `` $` `` y `$'` en la cadena
 * de reemplazo: una canción llamada «Rock $& Roll» se mostraba como
 * «Rock {0} Roll», y una con `$'` perdía el texto que venía después. Con una
 * función el valor entra literal.
 */
export function interpolar(plantilla: string, ...valores: unknown[]): string {
	return valores.reduce<string>(
		(texto, valor, indice) => texto.replaceAll(`{${indice}}`, () => String(valor)),
		plantilla
	);
}

/**
 * Elige entre singular y plural.
 *
 * El plugin no tiene plurales, así que van dos claves con sufijo `One`/`Other`
 * y la vista elige. Sin esto se termina mostrando «1 pistas».
 */
export function claveSegunCantidad(base: string, cantidad: number): string {
	return cantidad === 1 ? `${base}One` : `${base}Other`;
}
