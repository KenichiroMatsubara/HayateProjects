import { defineConfig } from 'vite';

import { appBundle } from '@torimi/bundle/vite';
import { tsubameSolid } from '@torimi/tsubame-solid/vite';

// Torimi App Bundle（単一 IIFE）を preset 2 部品の合成で作る（ADR-0008 §5, #769）:
//   - FW 変換: `@torimi/tsubame-solid/vite`（solid-js/universal → @torimi/tsubame-solid）
//   - App Bundle 形状: `@torimi/bundle/vite`（単一 IIFE・es2020・非圧縮・DOM/HTML なし）
//
// 出力は target 非依存の 1 本（dist-torimi/bundle.js）。native の Hermes 降格と降格済み別パス
// 配信は torimi CLI（#770）がビルド後に施すので、この config には target 分岐が無い。
export default defineConfig({
  plugins: [tsubameSolid()],
  ...appBundle({ entry: new URL('./src/main.bundle.tsx', import.meta.url), name: 'TsubameTodo', outDir: 'dist-torimi' }),
});
