import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Tauri expects a fixed dev port and a static build in dist/.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1450,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/target/**', '**/target/**', '**/crates/**'] },
  },
  worker: {
    format: 'es',
  },
  build: {
    target: 'es2022',
    outDir: 'dist',
    emptyOutDir: true,
  },
});
