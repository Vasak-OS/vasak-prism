<script setup lang="ts">
/**
 * La ventana del lanzador: un campo, una lista, y el teclado.
 *
 * Tres cosas que no se ven en el dibujo y son la mitad del trabajo:
 *
 *  1. **Cada consulta lleva un número.** Las respuestas pueden llegar
 *     desordenadas —la que salió antes tarda más— y sin el número la vieja pisa
 *     a la nueva: se ve la lista de «fir» después de haber escrito «firefox».
 *  2. **La espera antes de buscar es corta**, 40 ms, porque el trabajo está en
 *     Rust. No es para no cargar al backend: es para no pintar una lista por
 *     cada tecla de una palabra escrita rápido.
 *  3. **El catálogo avisa cuando cambia.** Instalar algo mientras la ventana
 *     está abierta rehace la búsqueda sin que nadie toque nada.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import ListaDeResultados from '@/componentes/ListaDeResultados.vue';
import { buscar, lanzar, type Resultado } from '@/servicios/busqueda';

/** Lo que se espera entre la tecla y la consulta. */
const ESPERA = 40;

const { t } = useI18n();

const consulta = ref('');
const resultados = ref<Resultado[]>([]);
const elegida = ref(0);
const campo = ref<HTMLInputElement | null>(null);

let numeroDeConsulta = 0;
let temporizador: ReturnType<typeof setTimeout> | null = null;
let soltarElAviso: UnlistenFn | null = null;

async function consultar(texto: string) {
	const mia = ++numeroDeConsulta;

	if (!texto.trim()) {
		resultados.value = [];
		elegida.value = 0;
		return;
	}

	try {
		const filas = await buscar(texto);
		// La respuesta de una consulta que ya no es la última se descarta. Sin
		// esto se ve la lista de «fir» después de haber escrito «firefox».
		if (mia !== numeroDeConsulta) return;
		resultados.value = filas;
		elegida.value = 0;
	} catch {
		if (mia !== numeroDeConsulta) return;
		resultados.value = [];
	}
}

function alEscribir(evento: Event) {
	consulta.value = (evento.target as HTMLInputElement).value;

	if (temporizador) clearTimeout(temporizador);
	temporizador = setTimeout(() => consultar(consulta.value), ESPERA);
}

function mover(cuanto: number) {
	const cuantos = resultados.value.length;
	if (cuantos === 0) return;
	// Da la vuelta: bajar desde la última lleva a la primera. En una lista corta
	// es más rápido que volver arriba a mano.
	elegida.value = (elegida.value + cuanto + cuantos) % cuantos;
}

async function elegir(indice: number) {
	const resultado = resultados.value[indice];
	if (!resultado) return;

	try {
		await lanzar(resultado);
	} catch (error) {
		// Se esconde igual: dejar la ventana abierta con la consulta puesta
		// parece que no se apretó nada. Lo que falló va al diario y no a la cara
		// del usuario, que a esta altura ya está mirando otra cosa.
		console.error('No se pudo lanzar', resultado.id, error);
	}

	await cerrar();
}

async function cerrar() {
	consulta.value = '';
	resultados.value = [];
	elegida.value = 0;
	numeroDeConsulta++;
	// Esconder y no cerrar: la ventana se construye una vez y se muestra, que es
	// lo que hace que abrir el lanzador sea instantáneo.
	await getCurrentWindow().hide();
}

function alTeclear(evento: KeyboardEvent) {
	switch (evento.key) {
		case 'ArrowDown':
			evento.preventDefault();
			mover(1);
			break;
		case 'ArrowUp':
			evento.preventDefault();
			mover(-1);
			break;
		case 'Enter':
			evento.preventDefault();
			elegir(elegida.value);
			break;
		case 'Escape':
			evento.preventDefault();
			cerrar();
			break;
	}
}

onMounted(async () => {
	await nextTick();
	campo.value?.focus();

	soltarElAviso = await listen('catalogo-cambiado', () => consultar(consulta.value));
});

onBeforeUnmount(() => {
	if (temporizador) clearTimeout(temporizador);
	soltarElAviso?.();
});
</script>

<template>
  <div class="flex h-screen w-screen items-start justify-center p-6">
    <div
      class="flex max-h-[70vh] w-full max-w-[640px] flex-col overflow-hidden rounded-window border border-ui-border bg-ui-bg/90 shadow-xl">
      <div class="flex items-center gap-3 border-b border-ui-border px-4 py-3">
        <input
          ref="campo"
          type="text"
          class="w-full bg-transparent text-lg text-tx-main outline-none placeholder:text-tx-main/40"
          :placeholder="t('lanzador.escribi')"
          :value="consulta"
          :aria-label="t('lanzador.escribi')"
          autocomplete="off"
          spellcheck="false"
          @input="alEscribir"
          @keydown="alTeclear">
      </div>

      <ListaDeResultados
        v-if="resultados.length > 0"
        :resultados="resultados"
        :elegida="elegida"
        @elegir="elegir"
        @apuntar="(indice: number) => (elegida = indice)" />

      <p v-else-if="consulta.trim()" class="px-4 py-6 text-center text-sm text-tx-main/50">
        {{ t('lanzador.nada') }}
      </p>
    </div>
  </div>
</template>
