import { getIconSource } from '@vasakgroup/plugin-vicons';
import { setupContextMenu } from '@vasakgroup/plugin-vsk-contextual-menu';
import { captureFailures } from '@vasakgroup/plugin-vsk-journal';
import I18n from '@vasakgroup/tauri-plugin-i18n';
import { createPinia } from 'pinia';
import { createApp } from 'vue';
import App from '@/App.vue';
import { sanearUrl } from '@/tools/csp';
import '@/assets/main.css';

/**
 * Cuánto se espera a las traducciones antes de montar.
 *
 * Se espera para que la primera pantalla no muestre las claves crudas, pero con
 * un plazo: si el backend no contesta, es mejor una interfaz con las claves a la
 * vista que una ventana en blanco para siempre.
 */
const PLAZO_TRADUCCIONES_MS = 3000;

// Una violación de CSP no se ve: el recurso no carga y la interfaz queda a
// medias sin decir nada. Esto la manda a la consola, saneada.
document.addEventListener('securitypolicyviolation', (evento) => {
	// El respaldo va **después** de sanear, no antes.
	//
	// Mirando el valor crudo, una entrada como `?token=X` es verdadera y pasa
	// el respaldo de largo — pero lo que queda de ella al sanearla es nada, así
	// que el registro salía con el campo en blanco. Sanear primero y decidir
	// después es lo que hace que un aviso incompleto no exista.
	const recurso = sanearUrl(evento.blockedURI) || '(en línea)';
	const origen = sanearUrl(evento.sourceFile) || 'documento';
	console.error(
		`[CSP] bloqueado ${recurso} por la directiva ` +
			`«${evento.violatedDirective}» en ${origen}:${evento.lineNumber}`
	);
});

// Lo que rompe la interfaz va al diario del sistema, con el nombre de esta
// aplicación. Antes no iba a ninguna parte: un error de JavaScript deja la
// pantalla a medias y no queda registro de por qué, y la consola del WebView no
// la ve nadie en una máquina instalada.
//
// Va lo más arriba posible, antes de armar la aplicación, para que también
// atrape lo que falle durante el arranque.
captureFailures();

// El clic derecho abre el menú de VasakOS —el mismo de todo el escritorio— y no
// el del motor del navegador, que ofrece «Recargar» e «Inspeccionar elemento».
setupContextMenu({ iconResolver: getIconSource });

const app = createApp(App);
const pinia = createPinia();

app.use(pinia);

// Un error de Vue en producción no va a ninguna parte; al menos que quede en la
// consola con el contexto de dónde ocurrió.
app.config.errorHandler = (error, _instancia, info) => {
	console.error(`[vue] falló en ${info}:`, error);
};

// Se esperan las traducciones antes de montar, con plazo: montando primero, el
// arranque enseña las claves crudas hasta que el archivo de idioma termina de
// cargar.
await Promise.race([
	I18n.getInstance()
		.load()
		.catch((error) => {
			console.error('No se pudieron cargar las traducciones', error);
		}),
	new Promise((resolve) => setTimeout(resolve, PLAZO_TRADUCCIONES_MS)),
]);

app.mount('#app');
