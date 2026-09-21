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
 *
 * La ventana no se cierra nunca: se esconde. Se construye al levantar la sesión
 * y vive escondida, que es de lo que depende que abrir el lanzador sea
 * instantáneo. Por eso hay que limpiar al aparecer y no al montar — el montaje
 * pasa una vez y la apertura, cientos.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import ListaDeResultados from '@/componentes/ListaDeResultados.vue';
import {
	buscar,
	elegir as elegirEnElBackend,
	esconder,
	type Resultado,
} from '@/servicios/busqueda';

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
let soltarElAparecer: UnlistenFn | null = null;

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

	// Una fila de completar no hace nada: escribe en el campo y deja seguir. Es
	// para cuando lo escrito todavía no alcanza —un bang a medias— y ofrecerle a
	// alguien que siga escribiendo es más útil que no ofrecerle nada.
	if (resultado.origen === 'completar') {
		consulta.value = resultado.id;
		campo.value?.focus();
		await consultar(consulta.value);
		return;
	}

	try {
		await elegirEnElBackend(resultado);
	} catch (error) {
		// Se esconde igual: dejar la ventana abierta con la consulta puesta
		// parece que no se apretó nada. Lo que falló va al diario y no a la cara
		// del usuario, que a esta altura ya está mirando otra cosa.
		console.error('No se pudo abrir', resultado.id, error);
	}

	await cerrar();
}

/** Deja la ventana como recién abierta, sin tocarla. */
function limpiar() {
	consulta.value = '';
	resultados.value = [];
	elegida.value = 0;
	// La consulta que estuviera en vuelo ya no interesa: si contestara después
	// de reabrir, aparecerían los resultados de la búsqueda anterior sobre un
	// campo vacío.
	numeroDeConsulta++;
}

async function cerrar() {
	limpiar();
	await esconder();
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

	// Cada vez que la ventana aparece. El foco hay que ponerlo de nuevo: la
	// superficie estuvo escondida y el campo lo perdió.
	soltarElAparecer = await listen('prism:mostrada', async () => {
		limpiar();
		await nextTick();
		campo.value?.focus();
	});
});

onBeforeUnmount(() => {
	if (temporizador) clearTimeout(temporizador);
	soltarElAviso?.();
	soltarElAparecer?.();
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

      <!-- Con el campo vacío el panel no mostraba nada, y era el único momento
           en que hay lugar para decir que los prefijos existen. Una sintaxis
           que no se ve es una sintaxis que no se usa: es lo que le pasó a la
           búsqueda vieja, que no tenía ni atajo. -->
      <p v-else class="px-4 py-4 text-center text-xs text-tx-main/40">
        {{ t('lanzador.prefijos') }}
      </p>
    </div>
  </div>
</template>
