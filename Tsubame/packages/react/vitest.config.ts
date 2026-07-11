import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

// 自パッケージの `@torimi/tsubame-react/jsx-(dev-)runtime` を dist ではなく src へ解決する。
const jsxRuntime = fileURLToPath(new URL('./src/jsx-runtime.ts', import.meta.url));
const jsxDevRuntime = fileURLToPath(new URL('./src/jsx-dev-runtime.ts', import.meta.url));

export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
    exclude: ['**/node_modules/**'],
  },
  esbuild: {
    jsx: 'automatic',
    jsxImportSource: '@torimi/tsubame-react',
  },
  resolve: {
    alias: {
      // dev を先に並べて prefix 衝突を避ける
      '@torimi/tsubame-react/jsx-dev-runtime': jsxDevRuntime,
      '@torimi/tsubame-react/jsx-runtime': jsxRuntime,
    },
  },
});
