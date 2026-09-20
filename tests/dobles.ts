/**
 * Los dobles de lo que sólo existe adentro de la ventana de Tauri.
 *
 * Sin ellos, importar el layout falla en la primera línea: el marco pide
 * iconos, escucha el cambio de tema y lee la configuración del escritorio.
 */

export const laVentanaRecibio: string[] = [];

export function getCurrentWindow() {
	return {
		minimize: async () => void laVentanaRecibio.push('minimize'),
		toggleMaximize: async () => void laVentanaRecibio.push('toggleMaximize'),
		close: async () => void laVentanaRecibio.push('close'),
	};
}

/** El `t()` devuelve la clave: una prueba que mire el texto mira la clave. */
export function useI18n() {
	return { t: (clave: string) => clave, locale: { value: 'es' } };
}

/** Lo que el marco lee para saber de qué lado va la barra. */
let configuracion: Record<string, unknown> = {};

export function ponerLaConfiguracion(nueva: Record<string, unknown>) {
	configuracion = nueva;
}

export async function readConfig() {
	return configuracion;
}

export function useConfigStore() {
	return { config: configuracion, loadConfig: async () => {} };
}

export async function invoke(_comando: string) {
	return undefined;
}

export async function listen(_nombre: string, _manejador: () => unknown) {
	return () => {};
}

export async function getIconSource(_nombre: string) {
	return '';
}

export async function getSymbolSource(_nombre: string) {
	return '';
}

export function olvidarTodo() {
	laVentanaRecibio.length = 0;
	configuracion = {};
}
