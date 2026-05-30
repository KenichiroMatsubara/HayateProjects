import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// tsubame-solid は solid-js/universal のカスタムレンダラー。JSX は
// generate: 'universal' でコンパイルし、生成される関数 import 先を
// '@tsubame/solid' に向ける（moduleName）。
export default defineConfig({
  plugins: [
    solid({
      solid: {
        moduleName: '@tsubame/solid',
        generate: 'universal',
      },
    }),
  ],
});
