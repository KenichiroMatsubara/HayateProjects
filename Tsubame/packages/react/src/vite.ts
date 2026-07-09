import type { UserConfig } from 'vite';

// FW 変換 preset（ADR-0008 §5）。React automatic runtime の JSX を Tsubame Renderer
// Protocol へ向け替える（`jsxImportSource` を `@tsubame/react` に向ける）vite 設定断片を
// 返す。solid 版（`@tsubame/solid/vite`）と対称に、FW 固有の知識を adapter パッケージへ
// 局在させる。
//
// `process.env.NODE_ENV` の静的置換も同梱する: React は prod/dev エントリを
// `process.env.NODE_ENV` で分岐するが、App Bundle はブラウザ host の `eval` や Hermes で
// 実行され `process` が無いため、未置換だと `ReferenceError: process is not defined` で
// 起動が落ちる。production 実体へ静的置換して塞ぐ（solid は `process` を参照しないので不要）。
export function tsubameReact(): UserConfig {
  return {
    esbuild: {
      jsx: 'automatic',
      jsxImportSource: '@tsubame/react',
    },
    define: {
      'process.env.NODE_ENV': JSON.stringify('production'),
    },
  };
}

export default tsubameReact;
