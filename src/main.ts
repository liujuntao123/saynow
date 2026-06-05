import { createApp, nextTick } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import App from './App.vue';
import RecorderOverlay from './RecorderOverlay.vue';
import './styles.css';

const isRecorderView = new URLSearchParams(window.location.search).get('view') === 'recorder';
const root = isRecorderView ? RecorderOverlay : App;

createApp(root).mount('#app');

if (!isRecorderView && '__TAURI_INTERNALS__' in window) {
  void nextTick().then(() => {
    window.requestAnimationFrame(() => {
      void getCurrentWindow()
        .show()
        .then(() => getCurrentWindow().setFocus())
        .catch((error) => console.error('[saynow] failed to show main window', error));
    });
  });
}
