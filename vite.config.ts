import { defineConfig } from 'vite';
import vue from '@vitejs/plugin-vue';

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    environment: 'node',
    exclude: ['**/node_modules/**', '**/dist/**', '**/src-tauri/target/**', '**/附件/**'],
  },
});
