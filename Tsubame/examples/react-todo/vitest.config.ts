import { defineConfig } from 'vitest/config';

// react-todo のユニットテスト。App Bundle エントリ（main.bundle.tsx）が JSX を含むため、
// ブラウザビルド（vite.config.ts）と同じ automatic runtime / jsxImportSource で変換する。
export default defineConfig({
  esbuild: {
    jsx: 'automatic',
    jsxImportSource: '@tsubame/react',
  },
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts', 'src/**/*.test.tsx'],
  },
});
