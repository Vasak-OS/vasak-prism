<script lang="ts">
/**
 * Lo que mide cada fila, en píxeles.
 *
 * Todas miden lo mismo, que es lo que permite calcular qué filas se ven sin
 * medir ninguna. Va exportado para que la prueba del alto lo lea de acá y no
 * de una copia: es el número que no puede moverse al cambiar el aspecto de la
 * fila (56 antes y después de pasar a `ListRow`, medido en el banco).
 */
export const RESULT_ROW_HEIGHT = 56;

/** El `id` de la lista, para que el campo diga que la maneja. */
export const RESULT_LIST_ID = 'prism-results';

/** El `id` de cada opción, para `aria-activedescendant`. */
export function resultOptionId(index: number): string {
	return `${RESULT_LIST_ID}-${index}`;
}
</script>

<script setup lang="ts">
/**
 * La lista de resultados, dibujando sólo lo que se ve.
 *
 * Cincuenta resultados en el DOM para mostrar ocho son cincuenta filas y
 * cincuenta iconos pedidos al tema, en cada tecla. Acá se dibuja la ventana
 * visible y un colchón, y el resto es un hueco con la altura que corresponde
 * para que la barra de desplazamiento siga valiendo.
 *
 * El alto de la ventana se mide con `ResizeObserver` y no con el evento
 * `resize`: adentro del WebView de GTK ese evento no llega, así que la lista se
 * quedaría con el alto del primer dibujo para siempre.
 */
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { desplazamientoParaVer, ventanaDeFilas } from '@/composables/useVentanaDeFilas';
import type { Resultado } from '@/servicios/busqueda';
import ResultRow from './ResultRow.vue';

const props = defineProps<{
	results: Resultado[];
	selected: number;
}>();

const emit = defineEmits<{ choose: [index: number]; point: [index: number] }>();

const box = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const height = ref(0);
let observer: ResizeObserver | null = null;

const rows = computed(() =>
	ventanaDeFilas(props.results.length, RESULT_ROW_HEIGHT, height.value, scrollTop.value)
);

const visible = computed(() => props.results.slice(rows.value.primera, rows.value.fin));

function onScroll() {
	scrollTop.value = box.value?.scrollTop ?? 0;
}

onMounted(() => {
	if (!box.value) return;
	height.value = box.value.clientHeight;
	observer = new ResizeObserver(() => {
		height.value = box.value?.clientHeight ?? 0;
	});
	observer.observe(box.value);
});

onBeforeUnmount(() => {
	observer?.disconnect();
	observer = null;
});

// Que la fila elegida se vea. No con `scrollIntoView`: con la lista
// virtualizada la fila puede no estar todavía en el DOM, así que hay que mover
// primero y dibujar después.
watch(
	() => props.selected,
	(index) => {
		if (!box.value || index < 0) return;
		const target = desplazamientoParaVer(index, RESULT_ROW_HEIGHT, height.value, scrollTop.value);
		if (target !== scrollTop.value) {
			box.value.scrollTop = target;
			scrollTop.value = target;
		}
	}
);

// Una consulta nueva es una lista nueva: dejarla desplazada donde estaba deja
// la primera fila —que es la elegida— fuera de la vista.
watch(
	() => props.results,
	() => {
		if (box.value) box.value.scrollTop = 0;
		scrollTop.value = 0;
	}
);
</script>

<template>
  <div
    :id="RESULT_LIST_ID"
    ref="box"
    class="min-h-0 flex-1 overflow-y-auto px-2 pb-2"
    role="listbox"
    @scroll="onScroll">
    <div class="relative w-full" :style="{ height: `${rows.altoTotal}px` }">
      <div
        class="absolute inset-x-0 top-0 flex flex-col"
        :style="{ transform: `translateY(${rows.desplazamientoDeLaPrimera}px)` }">
        <ResultRow
          v-for="(result, position) in visible"
          :id="resultOptionId(rows.primera + position)"
          :key="`${result.id}#${result.accion ?? ''}`"
          :result="result"
          :height="RESULT_ROW_HEIGHT"
          :selected="rows.primera + position === selected"
          @choose="emit('choose', rows.primera + position)"
          @point="emit('point', rows.primera + position)" />
      </div>
    </div>
  </div>
</template>
