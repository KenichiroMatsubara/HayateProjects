import { fileURLToPath } from 'node:url';

import { defineConfig } from 'vite';

import { tsubameSolid } from '@torimi/tsubame-solid/vite';

// #697 専用: `?renderer=vello` が読み込む `@torimi/hayate-adapter-web` の解決先を、既定の
// `Hayate/wasm-pkgs/pkg`（layer-present feature OFF）から `Hayate/wasm-pkgs/pkg-layer-present`
// （同じ default features + `layer-present` feature のみ追加、`pnpm --filter hayate
// build:layer-present` が生成）へ差し替える。`@torimi/hayate-host` 側の bare specifier import
// （`await import('@torimi/hayate-adapter-web')`）はそのままに、alias で物理的な解決先だけを
// 切り替えるので本番コードには一切手を入れない（vite.config.ts と地続きの通常ビルド）。
export default defineConfig({
  plugins: [tsubameSolid()],
  resolve: {
    alias: {
      '@torimi/hayate-adapter-web': fileURLToPath(
        new URL('../../../Hayate/wasm-pkgs/pkg-layer-present/hayate_adapter_web.js', import.meta.url),
      ),
    },
  },
});
