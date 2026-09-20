<script setup lang="ts">
/**
 * Una fila de la lista.
 *
 * El icono se pide por **nombre** al tema del escritorio, nunca por ruta: el
 * tema cambia en caliente y sus rutas no son estables. `ThemeIcon` memoriza lo
 * resuelto y comparte un solo oyente del cambio de tema entre todas las filas,
 * que es lo que hace que desplazar la lista no cueste dos llamadas por fila.
 */
import { ThemeIcon } from '@vasakgroup/vue-libvasak';
import type { Resultado } from '@/servicios/busqueda';

defineProps<{
	resultado: Resultado;
	elegida: boolean;
	alto: number;
}>();

defineEmits<{ elegir: []; apuntar: [] }>();
</script>

<template>
  <!-- El `mousedown.prevent` deja el foco en el campo de texto: sin él, hacer
       clic en una fila se lo saca y la próxima tecla no escribe en ningún lado. -->
  <div
    class="flex w-full items-center gap-3 rounded-corner px-3 transition-colors duration-100"
    :class="elegida ? 'bg-primary/20 ring-1 ring-primary/40' : 'hover:bg-ui-surface/70'"
    :style="{ height: `${alto}px` }"
    role="option"
    :aria-selected="elegida"
    @mousedown.prevent
    @click="$emit('elegir')"
    @mouseenter="$emit('apuntar')">
    <ThemeIcon v-if="resultado.icono" :name="resultado.icono" :size="32" />
    <!-- Un hueco del mismo tamaño cuando no hay icono, para que el texto de las
         filas quede alineado y la lista no se vea rota. -->
    <span v-else class="size-8 shrink-0" />

    <span class="flex min-w-0 flex-col text-left">
      <span class="truncate text-sm font-medium text-tx-main">{{ resultado.titulo }}</span>
      <span v-if="resultado.subtitulo" class="truncate text-xs text-tx-main/60">
        {{ resultado.subtitulo }}
      </span>
    </span>
  </div>
</template>
