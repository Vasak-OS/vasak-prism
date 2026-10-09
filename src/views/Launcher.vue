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
import { SearchField } from '@vasakgroup/vue-libvasak';
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import ResultList, { RESULT_LIST_ID, resultOptionId } from '@/components/ResultList.vue';
import { buscar, elegir as chooseInBackend, esconder, type Resultado } from '@/servicios/busqueda';

/** Lo que se espera entre la tecla y la consulta. */
const DELAY = 40;

const { t } = useI18n();

const query = ref('');
const results = ref<Resultado[]>([]);
const selected = ref(0);
const field = ref<InstanceType<typeof SearchField> | null>(null);

/** La opción marcada, para que un lector de pantalla la diga sin sacar el foco del campo. */
const activeOptionId = computed(() =>
	results.value.length > 0 ? resultOptionId(selected.value) : undefined
);

let queryNumber = 0;
let timer: ReturnType<typeof setTimeout> | null = null;
let stopCatalogListener: UnlistenFn | null = null;
let stopShownListener: UnlistenFn | null = null;

async function runQuery(text: string) {
	const mine = ++queryNumber;

	if (!text.trim()) {
		results.value = [];
		selected.value = 0;
		return;
	}

	try {
		const rows = await buscar(text);
		// La respuesta de una consulta que ya no es la última se descarta. Sin
		// esto se ve la lista de «fir» después de haber escrito «firefox».
		if (mine !== queryNumber) return;
		results.value = rows;
		selected.value = 0;
	} catch {
		if (mine !== queryNumber) return;
		results.value = [];
	}
}

function onType(text: string) {
	query.value = text;

	if (timer) clearTimeout(timer);
	timer = setTimeout(() => runQuery(query.value), DELAY);
}

function move(step: number) {
	const count = results.value.length;
	if (count === 0) return;
	// Da la vuelta: bajar desde la última lleva a la primera. En una lista corta
	// es más rápido que volver arriba a mano.
	selected.value = (selected.value + step + count) % count;
}

async function choose(index: number) {
	const result = results.value[index];
	if (!result) return;

	// Una fila de completar no hace nada: escribe en el campo y deja seguir. Es
	// para cuando lo escrito todavía no alcanza —un bang a medias— y ofrecerle a
	// alguien que siga escribiendo es más útil que no ofrecerle nada.
	if (result.origen === 'completar') {
		query.value = result.id;
		field.value?.focus();
		await runQuery(query.value);
		return;
	}

	try {
		await chooseInBackend(result);
	} catch (error) {
		// Se esconde igual: dejar la ventana abierta con la consulta puesta
		// parece que no se apretó nada. Lo que falló va al diario y no a la cara
		// del usuario, que a esta altura ya está mirando otra cosa.
		console.error('No se pudo abrir', result.id, error);
	}

	await close();
}

/** Deja la ventana como recién abierta, sin tocarla. */
function reset() {
	query.value = '';
	results.value = [];
	selected.value = 0;
	// La consulta que estuviera en vuelo ya no interesa: si contestara después
	// de reabrir, aparecerían los resultados de la búsqueda anterior sobre un
	// campo vacío.
	queryNumber++;
}

async function close() {
	reset();
	await esconder();
}

function onKey(event: KeyboardEvent) {
	switch (event.key) {
		case 'ArrowDown':
			event.preventDefault();
			move(1);
			break;
		case 'ArrowUp':
			event.preventDefault();
			move(-1);
			break;
		case 'Enter':
			event.preventDefault();
			choose(selected.value);
			break;
		case 'Escape':
			event.preventDefault();
			close();
			break;
	}
}

onMounted(async () => {
	await nextTick();
	field.value?.focus();

	stopCatalogListener = await listen('catalogo-cambiado', () => runQuery(query.value));

	// Cada vez que la ventana aparece. El foco hay que ponerlo de nuevo: la
	// superficie estuvo escondida y el campo lo perdió.
	stopShownListener = await listen('prism:mostrada', async () => {
		reset();
		await nextTick();
		field.value?.focus();
	});
});

onBeforeUnmount(() => {
	if (timer) clearTimeout(timer);
	stopCatalogListener?.();
	stopShownListener?.();
});
</script>

<template>
  <div class="flex h-screen w-screen items-start justify-center p-6">
    <!-- La superficie del escritorio: `ui-shell`, el fondo de la ventana al
         85 % y sin `backdrop-blur`, con el canto de afuera que se elige en
         Configuración (`window-border`) y la sombra de Once UI.
         El desenfoque de lo de atrás lo pone Wayfire, y sólo se ve si la
         superficie deja pasar algo: en `ui-float`, opaca, lo tapaba
         (vue-libvasak `docs/once-ui.md` §13). -->
    <div
      class="flex max-h-[70vh] w-full max-w-[640px] flex-col overflow-hidden rounded-corner-window window-border bg-ui-shell shadow-surface-l">
      <!-- `py-1.5` con el campo de 40 da los mismos 52 px de alto que tenía la
           cabecera con `py-3` y el texto grande a mano: cambia el campo, no el
           formato del panel. -->
      <!-- `spellcheck` se hereda: va en la cabecera porque el campo de la
           librería no lo declara, y subrayar en rojo «firefox» no ayuda. -->
      <div class="flex items-center gap-3 border-b border-ui-line-weak px-2 py-1.5" spellcheck="false">
        <SearchField
          ref="field"
          class="w-full"
          size="lg"
          bare
          :clearable="false"
          :model-value="query"
          :placeholder="t('lanzador.escribi')"
          :label="t('lanzador.escribi')"
          :listbox-id="results.length > 0 ? RESULT_LIST_ID : undefined"
          :active-option-id="activeOptionId"
          :expanded="results.length > 0"
          @update:model-value="onType"
          @keydown="onKey" />
      </div>

      <ResultList
        v-if="results.length > 0"
        :results="results"
        :selected="selected"
        @choose="choose"
        @point="(index: number) => (selected = index)" />

      <p v-else-if="query.trim()" class="px-4 py-6 text-center text-sm text-tx-muted">
        {{ t('lanzador.nada') }}
      </p>

      <!-- Con el campo vacío el panel no mostraba nada, y era el único momento
           en que hay lugar para decir que los prefijos existen. Una sintaxis
           que no se ve es una sintaxis que no se usa: es lo que le pasó a la
           búsqueda vieja, que no tenía ni atajo. -->
      <p v-else class="px-4 py-4 text-center text-xs text-tx-muted">
        {{ t('lanzador.prefijos') }}
      </p>
    </div>
  </div>
</template>
