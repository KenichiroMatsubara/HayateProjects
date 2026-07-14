import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    // 合成ルートと DOM 退避判定は純粋ロジック（DOM 非依存）。fake renderer / fake host で検証する。
    environment: 'node',
    exclude: ['**/node_modules/**'],
  },
});
