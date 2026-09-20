<script lang="ts" setup>
/**
 * La ventana de la aplicación.
 *
 * Es el molde del que nacen las aplicaciones de VasakOS, así que lo que esté
 * acá se copia dieciséis veces. Por eso no dibuja nada propio: el borde, la
 * esquina, el fondo, la barra y los tres botones de ventana salen de
 * `WindowFrame`, que es uno solo para todo el escritorio. Cuando el marco
 * cambia, cambian todas a la vez; cuando estaba copiado, cada una derivó por su
 * lado y dos ventanas del mismo escritorio dejaron de parecerse.
 *
 * # Dónde va la barra
 *
 * Donde diga `window.barPosition` en `~/.config/vasak/vasak.conf`: arriba,
 * abajo, a la izquierda o a la derecha. No hay nada que hacer para eso, lo
 * resuelve el marco, y lo que se ponga en la barra se acomoda solo si son
 * componentes de la librería —las pestañas, el buscador—.
 *
 * # Las ranuras
 *
 * `identidad` es el icono de la aplicación, `titulo` su nombre, `barra` lo que
 * la aplicación quiera poner en ella —pestañas, un selector—, `centro` lo que
 * vaya centrado respecto de la ventana entera, y `acciones` los botones que
 * valen para toda la ventana y van junto a los de la ventana.
 *
 * La ranura por omisión es el contenido. Sin ella,
 * `<WindowAppLayout>…</WindowAppLayout>` descartaba en silencio todo lo que se
 * le pusiera dentro y la ventana abría vacía: no hay ningún error, simplemente
 * no aparece nada. Costó una compilación y una captura darse cuenta.
 *
 * # Los botones de la ventana
 *
 * Van los tres, que es lo que corresponde a una aplicación normal. Una ventana
 * que no lo sea le pasa `:controls="[]"` al marco —un cuadro de diálogo se
 * responde, no se cierra— o la lista que necesite.
 */
import { useI18n } from '@vasakgroup/tauri-plugin-i18n';
import { WindowFrame } from '@vasakgroup/vue-libvasak';

const { t } = useI18n();
</script>

<template>
  <WindowFrame
    :minimize-label="t('ventana.minimizar')"
    :maximize-label="t('ventana.maximizar')"
    :close-label="t('ventana.cerrar')">
    <template v-if="$slots.identidad" #identidad><slot name="identidad" /></template>
    <template v-if="$slots.titulo" #titulo><slot name="titulo" /></template>
    <template v-if="$slots.barra" #barra><slot name="barra" /></template>
    <template v-if="$slots.centro" #centro><slot name="centro" /></template>
    <template v-if="$slots.acciones" #acciones><slot name="acciones" /></template>

    <div class="flex min-h-0 min-w-0 flex-1 p-1">
      <slot>
        <p class="p-4 text-sm text-tx-muted">
          Poné el contenido de la aplicación dentro de
          <code>&lt;WindowAppLayout&gt;</code>.
        </p>
      </slot>
    </div>
  </WindowFrame>
</template>
