import 'blockstitch/theme.css';
import './custom-block-color.css';
import './block-interactions.css';
import { createApp, watch } from 'vue';
import { useTheme } from 'blockstitch';
import App from './App.vue';
import { setupBlockstitch } from './blockstitchSetup';
import { setThemeBackground } from './tauri';

setupBlockstitch();
watch(useTheme().currentTheme, theme => void setThemeBackground(theme).catch(() => {}), { immediate: true });
createApp(App).mount('#app');
