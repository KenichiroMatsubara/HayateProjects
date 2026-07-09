import { defineConfig } from 'vitest/config';
import solid from 'vite-plugin-solid';

// todo のユニットテスト。App Bundle エントリ（main.bundle.tsx）が JSX を含むため、
// ブラウザビルド（vite.config.ts）と同じ solid-js/universal 変換で compile する
// （vitest.config.ts は vite.config.ts を継承しない — plugin はここにも要る）。
export default defineConfig({
  plugins: [
    solid({
      solid: {
        moduleName: '@tsubame/solid',
        generate: 'universal',
      },
    }),
  ],
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
