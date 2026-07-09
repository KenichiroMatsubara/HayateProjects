import { defineConfig } from 'vite';

import { appBundle } from '@torimi/bundle/vite';
import { tsubameSolid } from '@tsubame/solid/vite';

// Torimi App Bundle（単一 IIFE）を preset 2 部品の合成で作る（ADR-0008 §5, #769）:
//   - FW 変換: `@tsubame/solid/vite`（solid-js/universal → @tsubame/solid）
//   - App Bundle 形状: `@torimi/bundle/vite`（単一 IIFE・es2020・非圧縮・DOM/HTML なし）
//
// 単一エントリ化（#767）で native/web のバンドル差は「出力先」と「降格の有無」だけになった。
// 出力先はターゲットで切り替える（native = APK 同梱パス, web = dev-server 配信パス）。降格
// （Hermes lowering）はビルド後に torimi CLI（#770）が別パスへ施す責務なので、ここには無い。
// 従来の vite.config.android.ts はこの 1 本へ畳んだ。
const native = process.env.TORIMI_TARGET === 'native';
const output = native
  ? { outDir: 'dist-android', fileName: 'tsubame.js', name: 'TsubameTodoAndroid' }
  : { outDir: 'dist-torimi', fileName: 'bundle.js', name: 'TsubameTodoTorimi' };

export default defineConfig({
  plugins: [tsubameSolid()],
  ...appBundle({ entry: new URL('./src/main.bundle.tsx', import.meta.url), ...output }),
});
