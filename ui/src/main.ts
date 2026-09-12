import 'blockstitch/theme.css';
import './custom-block-color.css';
import { createApp } from 'vue';
import App from './App.vue';
import { setupBlockstitch } from './blockstitchSetup';

setupBlockstitch();
createApp(App).mount('#app');
