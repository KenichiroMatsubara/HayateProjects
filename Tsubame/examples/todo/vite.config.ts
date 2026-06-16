import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// tsubame-solid は solid-js/universal のカスタムレンダラー。JSX は
// generate: 'universal' でコンパイルし、生成される関数 import 先を
// '@tsubame/solid' に向ける（moduleName）。
export default defineConfig({
  // GitHub Pages の project site（/HayateProjects/ 配下）に置く場合は base を
  // 合わせないとアセットが 404 になる。ローカル dev/build には影響させず、
  // 環境変数で上書きする（Pages デプロイの workflow が VITE_BASE を設定）。
  base: process.env.VITE_BASE ?? '/',
  plugins: [
    solid({
      solid: {
        moduleName: '@tsubame/solid',
        generate: 'universal',
      },
    }),
  ],
});
