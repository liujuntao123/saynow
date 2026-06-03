import { createApp } from 'vue';
import App from './App.vue';
import RecorderOverlay from './RecorderOverlay.vue';
import './styles.css';

const root = new URLSearchParams(window.location.search).get('view') === 'recorder' ? RecorderOverlay : App;

createApp(root).mount('#app');
