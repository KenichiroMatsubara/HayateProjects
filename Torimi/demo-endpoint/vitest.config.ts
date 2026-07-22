import { cloudflareTest } from '@cloudflare/vitest-pool-workers';
import { defineConfig } from 'vitest/config';

// テストは workerd 上で実走させる（WebSocketPair・assets binding を本物で検証するため）。
// global setup が public/ にプレースホルダのデモバンドルを用意する（未ビルドでも hermetic に
// 走るように。ビルド済み実物があればそのまま使う）。
export default defineConfig({
  plugins: [
    cloudflareTest({
      wrangler: { configPath: './wrangler.jsonc' },
    }),
  ],
  test: {
    include: ['src/**/*.test.ts'],
    globalSetup: ['./test/global-setup.mjs'],
  },
});
