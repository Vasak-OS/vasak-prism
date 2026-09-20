import { invoke } from '@tauri-apps/api/core';

/** Una fila de la lista de resultados, tal cual la manda el backend. */
export interface Resultado {
	id: string;
	/** La acción de la entrada, si la fila es una acción y no la aplicación. */
	accion: string | null;
	titulo: string;
	subtitulo: string | null;
	/** El **nombre** del icono en el tema, nunca una ruta. */
	icono: string | null;
	puntaje: number;
}

export function buscar(consulta: string, limite?: number): Promise<Resultado[]> {
	return invoke<Resultado[]>('buscar', { consulta, limite });
}

export function lanzar(resultado: Resultado): Promise<void> {
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
