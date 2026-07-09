import { defineConfig } from 'vite';

import { tsubameSolid } from '@tsubame/solid/vite';

// ブラウザ向け web デモ（GitHub Pages）。FW 変換は `@tsubame/solid/vite` preset に集約し、
// moduleName / generate:'universal' の知識は adapter パッケージ側へ局在させる（#769）。
export default defineConfig({
  // GitHub Pages の project site（/HayateProjects/ 配下）に置く場合は base を
  // 合わせないとアセットが 404 になる。ローカル dev/build には影響させず、
  // 環境変数で上書きする（Pages デプロイの workflow が VITE_BASE を設定）。
  base: process.env.VITE_BASE ?? '/',
  plugins: [tsubameSolid()],
});
