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
		hide: async () => void laVentanaRecibio.push('hide'),
		show: async () => void laVentanaRecibio.push('show'),
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

/**
 * Lo que contesta cada comando del backend.
 *
 * Una función y no un valor para poder decidir desde la prueba **cuándo**
 * contesta: las carreras que el lanzador tiene que aguantar —una respuesta vieja
 * que llega última— sólo se pueden comprobar así.
 */
const respuestas = new Map<string, (args: Record<string, unknown>) => Promise<unknown>>();

/** Lo que se le pidió al backend, en orden. */
export const loQueSePidio: { comando: string; args: Record<string, unknown> }[] = [];

export function contestar(
	comando: string,
	como: (args: Record<string, unknown>) => Promise<unknown>
) {
	respuestas.set(comando, como);
}

export async function invoke(comando: string, args: Record<string, unknown> = {}) {
	loQueSePidio.push({ comando, args });
	const como = respuestas.get(comando);
	return como ? await como(args) : undefined;
}

const oyentes = new Map<string, Set<() => unknown>>();

export async function listen(nombre: string, manejador: () => unknown) {
	const suyos = oyentes.get(nombre) ?? new Set<() => unknown>();
	suyos.add(manejador);
	oyentes.set(nombre, suyos);
	return () => {
		suyos.delete(manejador);
	};
}

/** Emite un evento del escritorio y espera a que lo atiendan. */
export async function emitir(nombre: string) {
	for (const manejador of [...(oyentes.get(nombre) ?? [])]) {
		await manejador();
	}
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
	respuestas.clear();
	oyentes.clear();
	loQueSePidio.length = 0;
}
