<script setup lang="ts">
/**
 * La ventana.
 *
 * Sin marco ni barra de título: un lanzador no se minimiza ni se maximiza, se
 * muestra y se esconde. Por eso no usa `WindowAppLayout`, que es el molde de las
 * ventanas normales del escritorio; ese molde va a hacer falta cuando haya
 * preferencias, y por eso sigue en el repositorio.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useConfigStore } from '@vasakgroup/plugin-config-manager';
import { onMounted, onUnmounted, type Ref, ref } from 'vue';
import Lanzador from '@/vistas/Lanzador.vue';

const soltarLaConfiguracion: Ref<UnlistenFn | null> = ref(null);

onMounted(async () => {
	try {
		const configuracion = useConfigStore();
		await configuracion.loadConfig();

		soltarLaConfiguracion.value = await listen('config-changed', async () => {
			await configuracion.loadConfig();
		});
	} catch (error) {
		console.error('Error al cargar configuración en App.vue', error);
	}
});

onUnmounted(() => {
	soltarLaConfiguracion.value?.();
});
</script>

<template>
  <Lanzador />
</template>
