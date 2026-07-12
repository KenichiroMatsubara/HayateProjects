import solid from 'vite-plugin-solid';

import type { PluginOption } from 'vite';

// FW 変換 preset（ADR-0008 §5）。solid-js/universal を Tsubame Renderer Protocol へ
// 向け替える vite プラグインを返す。FW 固有の知識（生成関数の import 先 `moduleName` を
// `@torimi/tsubame-solid` に向ける・`generate: 'universal'`）を adapter パッケージに局在させ、
// 外部アプリの vite config を数行にする。App Bundle 形状（`@torimi/bundle/vite`）とは
// 直交し、両者を合成して 1 本の config にする。
export function tsubameSolid(): PluginOption {
  return solid({
    solid: {
      moduleName: '@torimi/tsubame-solid',
      generate: 'universal',
    },
  });
}

export default tsubameSolid;
