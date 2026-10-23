<script setup lang="ts">
/**
 * La ventana.
 *
 * Sin marco ni barra de título: un lanzador no se minimiza ni se maximiza, se
 * muestra y se esconde. Por eso no usa `WindowAppLayout`, que es el molde de las
 * ventanas normales del escritorio; ese molde va a hacer falta cuando haya
 * preferencias, y por eso sigue en el repositorio.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { useConfigStore } from '@vasakgroup/plugin-config-manager';
import { onMounted, onUnmounted, type Ref, ref } from 'vue';
import { saveTheme } from '@/tools/theme-cache';
import Launcher from '@/views/Launcher.vue';

const stopConfigListener: Ref<UnlistenFn | null> = ref(null);

onMounted(async () => {
	// Afina el WebKit de la ventana para bajar su consumo de memoria; sin bloquear.
	invoke('tune_webview').catch(() => {});

	try {
		const config = useConfigStore();
		await config.loadConfig().then(() => saveTheme());

		stopConfigListener.value = await listen('config-changed', async () => {
			await config.loadConfig().then(() => saveTheme());
		});
	} catch (error) {
		console.error('Error al cargar configuración en App.vue', error);
	}
});

onUnmounted(() => {
	stopConfigListener.value?.();
});
</script>

<template>
  <Launcher />
</template>
