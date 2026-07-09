import { defineConfig } from 'vite';

import { appBundle } from '@torimi/bundle/vite';
import { tsubameSolid } from '@tsubame/solid/vite';

// Torimi App Bundle（単一 IIFE）を preset 2 部品の合成で作る（ADR-0008 §5）:
//   - FW 変換: `@tsubame/solid/vite`（solid-js/universal → @tsubame/solid）
//   - App Bundle 形状: `@torimi/bundle/vite`（単一 IIFE・es2020・非圧縮・DOM/HTML なし）
export default defineConfig({
  plugins: [tsubameSolid()],
  // IIFE のグローバル名は wire 上は無関係（バンドルは `__torimiMount` で自己登録する）。
  // プロジェクト名（ハイフンを含みうる = JS 識別子として不正）を使わず固定の合法識別子にする。
  ...appBundle({ entry: new URL('./src/main.bundle.tsx', import.meta.url), name: 'TorimiApp', outDir: 'dist-torimi' }),
});
