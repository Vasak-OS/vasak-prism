import { invoke } from '@tauri-apps/api/core';

/**
 * De dónde salió una fila.
 *
 * Decide qué pasa al apretar Enter. Lo manda el backend en vez de dejar que la
 * interfaz lo adivine mirando la forma del resultado, que es como se termina
 * con dos lugares que tienen que estar de acuerdo.
 */
export type Origen = 'aplicacion' | 'calculo';

/** Una fila de la lista de resultados, tal cual la manda el backend. */
export interface Resultado {
	/** La aplicación, o el texto a copiar cuando la fila es una cuenta. */
	id: string;
	/** La acción de la entrada, si la fila es una acción y no la aplicación. */
	accion: string | null;
	titulo: string;
	subtitulo: string | null;
	/** El **nombre** del icono en el tema, nunca una ruta. */
	icono: string | null;
	puntaje: number;
	origen: Origen;
}

export function buscar(consulta: string, limite?: number): Promise<Resultado[]> {
	return invoke<Resultado[]>('buscar', { consulta, limite });
}

/**
 * Hace lo que corresponda con la fila elegida: abrir o copiar.
 *
 * El `switch` está acá y no en la vista porque es lo que sabe de la forma de un
 * resultado; la vista sólo sabe que se eligió uno.
 */
export function elegir(resultado: Resultado): Promise<void> {
	if (resultado.origen === 'calculo') {
		return invoke<void>('copiar', { texto: resultado.id });
	}

	return invoke<void>('lanzar', { id: resultado.id, accion: resultado.accion });
}

/**
 * Esconde la ventana.
 *
 * Pasa por el backend y no por `getCurrentWindow().hide()`: lo que se ve no es
 * la ventana de Tauri sino la superficie de capa a la que se mudó el WebView, y
 * el armazón de Tauri está escondido y vacío desde que arrancó. Esconderlo a él
 * no hace nada visible.
 */
export function esconder(): Promise<void> {
	return invoke<void>('esconder');
}
