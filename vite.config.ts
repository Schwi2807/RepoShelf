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
    // Never inline assets as data: URLs: the CSP has `font-src 'self'`, so a small inlined woff2 subset was blocked
    // in the real app (found by the M8 e2e, D-86). Every asset ships as its own file under dist/assets.
    assetsInlineLimit: 0,
  },
});
