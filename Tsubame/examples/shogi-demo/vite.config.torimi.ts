import { defineConfig, mergeConfig } from 'vite';

import { appBundle } from '@torimi/bundle/vite';
import { tsubameReact } from '@torimi/tsubame-react/vite';

// Torimi 将棋 App Bundle を preset 2 部品の合成で作る（#769）:
//   - FW 変換: `@torimi/tsubame-react/vite`
//   - App Bundle 形状: `@torimi/bundle/vite`（単一 IIFE・es2020・非圧縮・DOM/HTML なし）
//
// react-demo / solid-demo と対称。出力は target 非依存の 1 本（dist-torimi/bundle.js）で、
// native の Hermes 降格は torimi CLI の責務。
export default mergeConfig(
  tsubameReact(),
  defineConfig(
    appBundle({
      entry: new URL('./src/main.bundle.tsx', import.meta.url),
      name: 'TsubameShogi',
      outDir: 'dist-torimi',
    }),
  ),
);
