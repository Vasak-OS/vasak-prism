<script setup lang="ts">
/**
 * Una fila de la lista.
 *
 * Es la `ListRow` de la librería con `role="option"`: el velo de lo elegido, el
 * puntero y el anillo de foco salen de ahí, con la forma de Once UI
 * (vue-libvasak#74). Lo único que pone el lanzador es lo suyo:
 *
 * - **El alto.** Lo decide la lista, que es un desplazador virtual hecho a mano
 *   y calcula qué filas dibujar sin medir ninguna (`RESULT_ROW_HEIGHT`).
 *   `ListRow` no impone alto a propósito; acá va fijo en el estilo de la caja
 *   y la fila lo llena con `h-full`. Si midiera otra cosa, la barra de
 *   desplazamiento y la flecha del teclado dejarían de corresponder con lo que
 *   se ve.
 * - **El puntero y el foco.** `mouseenter` y `mousedown` van en la caja y no
 *   en `ListRow`, que no los declara: con `strictTemplates` un evento que el
 *   componente no declara es un error de tipos, y la salida de un `v-bind` de
 *   objeto no se comprueba.
 * - **El icono a 32.** `ListRow` lo pone a 24; el lanzador lo tenía a 32 y la
 *   pantalla no cambia de formato, así que va por la ranura `leading`.
 *
 * El `mousedown.prevent` deja el foco en el campo de texto: sin él, hacer clic
 * en una fila se lo saca y la próxima tecla no escribe en ningún lado. (Va acá
 * y no como comentario arriba de la raíz de la plantilla, que la parte en un
 * fragmento.)
 *
 * El icono se pide por **nombre** al tema del escritorio, nunca por ruta: el
 * tema cambia en caliente y sus rutas no son estables. `ThemeIcon` memoriza lo
 * resuelto y comparte un solo oyente del cambio de tema entre todas las filas,
 * que es lo que hace que desplazar la lista no cueste dos llamadas por fila.
 */

import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { ListRow, ThemeIcon } from '@vasakgroup/vue-libvasak';
import { computed } from 'vue';
import type { Origen, Resultado } from '@/servicios/busqueda';

const props = defineProps<{
	result: Resultado;
	selected: boolean;
	height: number;
	/** Para que el campo diga cuál está marcada (`aria-activedescendant`). */
	id: string;
}>();

defineEmits<{ choose: []; point: [] }>();

const { t } = useI18n();

/**
 * El subtítulo, traducido si corresponde.
 *
 * Los resultados que salen del disco traen texto de verdad —el nombre de la
 * aplicación, su descripción, la ruta de un archivo— y se muestran como vienen.
 * Los que arma el backend traen la **clave** del catálogo, porque el backend no
 * sabe en qué idioma está la sesión: ésos se traducen acá.
 */
const UNTRANSLATED: Origen[] = ['aplicacion', 'reciente'];

const subtitle = computed(() => {
	const own = props.result.subtitulo;
	if (!own) return '';
	if (UNTRANSLATED.includes(props.result.origen)) return own;

	// El `t()` del plugin no interpola: el marcador se reemplaza a mano.
	const translated = t(own);
	const datum = props.result.subtituloDato;
	return datum ? translated.replace('{0}', datum) : translated;
});
</script>

<template>
  <div
    class="flex flex-col"
    :style="{ height: `${height}px` }"
    @mousedown.prevent
    @mouseenter="$emit('point')">
    <ListRow
      :id="id"
      class="h-full"
      role="option"
      :selected="selected"
      :title="result.titulo"
      :description="subtitle || undefined"
      truncate
      @click="$emit('choose')">
      <template #leading>
        <ThemeIcon v-if="result.icono" :name="result.icono" :size="32" />
        <!-- Un hueco del mismo tamaño cuando no hay icono, para que el texto de
             las filas quede alineado y la lista no se vea rota. -->
        <span v-else class="size-8 shrink-0" />
      </template>
    </ListRow>
  </div>
</template>
