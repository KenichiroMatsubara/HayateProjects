import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// tsubame-solid は solid-js/universal のカスタムレンダラー。JSX は generate:
// 'universal' でコンパイルし、生成関数の import 先を '@tsubame/solid' に向ける。
// GitHub Pages の project site 配下に置く場合は VITE_BASE で base を上書きする
// （examples/todo と同じ流儀）。
export default defineConfig({
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
