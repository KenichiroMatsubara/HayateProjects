import { defineConfig } from 'vite';

import { tsubameSolid } from '@torimi/tsubame-solid/vite';

// 各デモを繋ぐ子 dev server のポート（それぞれの `dev:site` が `--port` で合わせる）。
const SHOGI_DEV_PORT = Number(process.env.SHOGI_DEV_PORT ?? 5186);
const REACT_DEV_PORT = Number(process.env.REACT_DEV_PORT ?? 5187);

// ブラウザ向け web デモ（GitHub Pages）。FW 変換は `@torimi/tsubame-solid/vite` preset に集約し、
// moduleName / generate:'universal' の知識は adapter パッケージ側へ局在させる（#769）。
export default defineConfig({
  // GitHub Pages の project site（/HayateProjects/ 配下）に置く場合は base を
  // 合わせないとアセットが 404 になる。ローカル dev/build には影響させず、
  // 環境変数で上書きする（Pages デプロイの workflow が VITE_BASE を設定）。
  base: process.env.VITE_BASE ?? '/',
  plugins: [tsubameSolid()],
  server: {
    // 開発中も Pages と同じ URL 配置（root = todo・/react = スケッチ・/shogi = 将棋）で
    // 見られるようにする。react 系は FW 変換（jsxImportSource）が solid と両立しないので
    // 同じ vite サーバでは配れない。別ポートに `base=/react/`・`base=/shogi/` で立てた
    // dev server へ丸ごと proxy する。base を合わせてあるので path 書き換えは不要で、
    // HMR の WS も `ws: true` でそのまま抜ける。
    // 子を起動していないときはそのサブパスだけが 502 になり、root の todo には影響しない。
    proxy: {
      '/react': {
        target: `http://localhost:${REACT_DEV_PORT}`,
        ws: true,
      },
      '/shogi': {
        target: `http://localhost:${SHOGI_DEV_PORT}`,
        ws: true,
      },
    },
  },
});
