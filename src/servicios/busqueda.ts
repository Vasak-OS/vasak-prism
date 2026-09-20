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
