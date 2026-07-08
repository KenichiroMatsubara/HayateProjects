import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// Android（埋め込み Hermes, ADR-0112）向けの単一ファイルバンドル。
//
// ブラウザ用 `vite.config.ts` と同じ solid-js/universal 変換を使いつつ、
// エントリを `main.android.tsx` にし、DOM/HTML を伴わない単一の IIFE として
// 出力する。生成物（`dist-android/tsubame.js`）を APK assets に同梱し、
// 起動時に Hermes へロードする（後段で hermesc により .hbc へ事前コンパイル）。
export default defineConfig({
  plugins: [
    solid({
      solid: {
        moduleName: '@tsubame/solid',
        generate: 'universal',
      },
    }),
  ],
  build: {
    // Hermes は匿名 class 式（`var X = class {}`）を正しく評価できないため、ビルド後に
    // scripts/lower-for-hermes.mjs（Babel preset-env）で class/modern 構文を ES5 相当へ
    // 降格する（torimi:native:build スクリプト, ADR-0112）。vite の target はここでは通常値。
    target: 'es2020',
    outDir: 'dist-android',
    emptyOutDir: true,
    cssCodeSplit: false,
    // デバッグしやすさ優先で非圧縮。サイズ最適化は後段（hermesc/リリース）で。
    minify: false,
    lib: {
      entry: fileURLToPath(new URL('./src/main.android.tsx', import.meta.url)),
      formats: ['iife'],
      name: 'TsubameTodoAndroid',
      fileName: () => 'tsubame.js',
    },
  },
});
