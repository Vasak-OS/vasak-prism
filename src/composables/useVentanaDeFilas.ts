/**
 * Qué filas de la lista hay que dibujar.
 *
 * Una consulta puede devolver cincuenta resultados y en la ventana entran ocho.
 * Dibujar las cincuenta significa cincuenta filas en el DOM y cincuenta iconos
 * pedidos al tema, en cada tecla, para mostrar ocho. Acá se calcula cuáles están
 * a la vista y se dibujan sólo ésas.
 *
 * La cuenta es aparte del componente y no usa nada del DOM: es la parte que se
 * puede equivocar —un resultado que no se ve, una fila que parpadea al
 * desplazar— y así se prueba sin montar nada.
 */

/** Filas de más que se dibujan arriba y abajo de lo que se ve. */
export const COLCHON = 3;

export interface Ventana {
	/** La primera fila a dibujar, contando desde cero. */
	primera: number;
	/** Una más que la última: para un `slice`. */
	fin: number;
	/** Lo que mide la lista entera, para que la barra de desplazamiento valga. */
	altoTotal: number;
	/** Dónde empieza la primera fila dibujada. */
	desplazamientoDeLaPrimera: number;
}

export function ventanaDeFilas(
	total: number,
	altoDeFila: number,
	altoVisible: number,
	desplazamiento: number,
	colchon = COLCHON
): Ventana {
	if (total <= 0 || altoDeFila <= 0) {
		return { primera: 0, fin: 0, altoTotal: 0, desplazamientoDeLaPrimera: 0 };
	}

	// El desplazamiento puede venir negativo —el rebote de algunos navegadores—
	// y eso daría una primera fila negativa y un `slice` vacío.
	const arriba = Math.max(0, desplazamiento);
	const cuantasEntran = Math.max(1, Math.ceil(altoVisible / altoDeFila));

	const primera = Math.max(0, Math.floor(arriba / altoDeFila) - colchon);
	const fin = Math.min(total, primera + cuantasEntran + colchon * 2);

	return {
		primera,
		fin,
		altoTotal: total * altoDeFila,
		desplazamientoDeLaPrimera: primera * altoDeFila,
	};
}

/**
 * Adónde hay que desplazar para que la fila elegida se vea entera.
 *
 * Devuelve el desplazamiento que corresponde, que es el mismo que había si la
 * fila ya se veía. No se usa `scrollIntoView` porque con la lista virtualizada
 * la fila puede no estar en el DOM todavía: hay que mover primero y dibujar
 * después.
 */
export function desplazamientoParaVer(
	indice: number,
	altoDeFila: number,
	altoVisible: number,
	desplazamiento: number
): number {
	const arriba = indice * altoDeFila;
	const abajo = arriba + altoDeFila;

	if (arriba < desplazamiento) {
		return arriba;
	}

	if (abajo > desplazamiento + altoVisible) {
		return abajo - altoVisible;
	}

	return desplazamiento;
}
