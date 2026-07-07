import { defineWorkersConfig } from '@cloudflare/vitest-pool-workers/config';

// テストは workerd 上で実走させる（WebSocketPair・assets binding を本物で検証するため）。
// global setup が public/ にプレースホルダのデモバンドルを用意する（未ビルドでも hermetic に
// 走るように。ビルド済み実物があればそのまま使う）。
export default defineWorkersConfig({
  test: {
    include: ['src/**/*.test.ts'],
    globalSetup: ['./test/global-setup.mjs'],
    poolOptions: {
      workers: {
        wrangler: { configPath: './wrangler.jsonc' },
      },
    },
  },
});
