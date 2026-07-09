import { defineConfig, mergeConfig } from 'vite';

import { appBundle } from '@torimi/bundle/vite';
import { tsubameReact } from '@tsubame/react/vite';

// Torimi react App Bundle（#531：FW 非依存の実証）を preset 2 部品の合成で作る（#769）:
//   - FW 変換: `@tsubame/react/vite`（automatic JSX → @tsubame/react, NODE_ENV=production）
//   - App Bundle 形状: `@torimi/bundle/vite`（単一 IIFE・es2020・非圧縮・DOM/HTML なし）
//
// solid 版（`examples/todo/vite.config.torimi.ts`）と対称：FW 固有の変換はバンドル側に閉じ、
// 出力する wire シームは同一なので同じホストが描画できる（ADR-0001）。単一エントリ化（#767）で
// native/web 差は出力先と降格の有無だけになり、従来の vite.config.android.ts はこの 1 本へ畳んだ。
const native = process.env.TORIMI_TARGET === 'native';
const output = native
  ? { outDir: 'dist-android', fileName: 'tsubame.js', name: 'TsubameReactTodoAndroid' }
  : { outDir: 'dist-torimi', fileName: 'bundle.js', name: 'TsubameReactTodoTorimi' };

export default mergeConfig(
  tsubameReact(),
  defineConfig(appBundle({ entry: new URL('./src/main.bundle.tsx', import.meta.url), ...output })),
);
