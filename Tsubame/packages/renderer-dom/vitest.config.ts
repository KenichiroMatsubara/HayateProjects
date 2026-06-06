import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@tsubame/renderer-protocol': path.resolve(
        import.meta.dirname,
        '../renderer-protocol/src/index.ts',
      ),
      '@tsubame/hayate-css-catalog': path.resolve(
        import.meta.dirname,
        '../hayate-css-catalog/src/index.ts',
      ),
    },
  },
  test: {
    environment: 'happy-dom',
    exclude: ['test/**', '**/node_modules/**'],
  },
});
