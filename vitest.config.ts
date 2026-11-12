import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'happy-dom',
    include: ['src/**/*.test.{ts,tsx}'],
    // M13: with 28 files in parallel workers on a 32-thread laptop, screen tests that render whole pages took 5–7 s
    // in some runs and hit the 5 s default (different tests each time, also without M13 changes).
    testTimeout: 20000,
  },
});
