import { defineConfig, mergeConfig } from 'vite';

import { appBundle } from '@torimi/bundle/vite';
import { tsubameReact } from '@torimi/tsubame-react/vite';

// Torimi react App Bundle（#531：FW 非依存の実証）を preset 2 部品の合成で作る（#769）:
//   - FW 変換: `@torimi/tsubame-react/vite`（automatic JSX → @torimi/tsubame-react, NODE_ENV=production）
//   - App Bundle 形状: `@torimi/bundle/vite`（単一 IIFE・es2020・非圧縮・DOM/HTML なし）
//
// solid 版（`examples/solid-demo/vite.config.torimi.ts`）と対称：FW 固有の変換はバンドル側に閉じ、
// 出力する wire シームは同一なので同じホストが描画できる（ADR-0001）。出力は target 非依存の
// 1 本（dist-torimi/bundle.js）で、native の Hermes 降格は torimi CLI（#770）の責務。
export default mergeConfig(
  tsubameReact(),
  defineConfig(
    appBundle({ entry: new URL('./src/main.bundle.tsx', import.meta.url), name: 'TsubameReactTodo', outDir: 'dist-torimi' }),
  ),
);
