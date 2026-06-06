import { defineConfig } from 'vitest/config';
import path from 'node:path';

export default defineConfig({
  resolve: {
    alias: {
      '@tsubame/renderer-protocol': path.resolve(
        import.meta.dirname,
        '../renderer-protocol/src/index.ts',
      ),
    },
  },
  test: {
    environment: 'node',
    exclude: ['test/**', '**/node_modules/**'],
  },
});
