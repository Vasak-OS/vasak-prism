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
import FilaDeResultado from './FilaDeResultado.vue';

/** Todas las filas miden lo mismo, que es lo que permite calcular sin medir. */
const ALTO_DE_FILA = 56;

const props = defineProps<{
	resultados: Resultado[];
	elegida: number;
}>();

const emit = defineEmits<{ elegir: [indice: number]; apuntar: [indice: number] }>();

const caja = ref<HTMLElement | null>(null);
const desplazamiento = ref(0);
const alto = ref(0);
let observador: ResizeObserver | null = null;

const ventana = computed(() =>
	ventanaDeFilas(props.resultados.length, ALTO_DE_FILA, alto.value, desplazamiento.value)
);

const visibles = computed(() => props.resultados.slice(ventana.value.primera, ventana.value.fin));

function alDesplazar() {
	desplazamiento.value = caja.value?.scrollTop ?? 0;
}

onMounted(() => {
	if (!caja.value) return;
	alto.value = caja.value.clientHeight;
	observador = new ResizeObserver(() => {
		alto.value = caja.value?.clientHeight ?? 0;
	});
	observador.observe(caja.value);
});

onBeforeUnmount(() => {
	observador?.disconnect();
	observador = null;
});

// Que la fila elegida se vea. No con `scrollIntoView`: con la lista
// virtualizada la fila puede no estar todavía en el DOM, así que hay que mover
// primero y dibujar después.
watch(
	() => props.elegida,
	(indice) => {
		if (!caja.value || indice < 0) return;
		const donde = desplazamientoParaVer(indice, ALTO_DE_FILA, alto.value, desplazamiento.value);
		if (donde !== desplazamiento.value) {
			caja.value.scrollTop = donde;
			desplazamiento.value = donde;
		}
	}
);

// Una consulta nueva es una lista nueva: dejarla desplazada donde estaba deja
// la primera fila —que es la elegida— fuera de la vista.
watch(
	() => props.resultados,
	() => {
		if (caja.value) caja.value.scrollTop = 0;
		desplazamiento.value = 0;
	}
);
</script>

<template>
  <div
    ref="caja"
    class="min-h-0 flex-1 overflow-y-auto px-2 pb-2"
    role="listbox"
    @scroll="alDesplazar">
    <div class="relative w-full" :style="{ height: `${ventana.altoTotal}px` }">
      <div
        class="absolute inset-x-0 top-0 flex flex-col"
        :style="{ transform: `translateY(${ventana.desplazamientoDeLaPrimera}px)` }">
        <FilaDeResultado
          v-for="(resultado, posicion) in visibles"
          :key="`${resultado.id}#${resultado.accion ?? ''}`"
          :resultado="resultado"
          :alto="ALTO_DE_FILA"
          :elegida="ventana.primera + posicion === elegida"
          @elegir="emit('elegir', ventana.primera + posicion)"
          @apuntar="emit('apuntar', ventana.primera + posicion)" />
      </div>
    </div>
  </div>
</template>
