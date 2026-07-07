import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

// Android（埋め込み Hermes, ADR-0112）向けの単一ファイルバンドル（react 版, #739）。
//
// solid 版（`examples/todo/vite.config.android.ts`）と同型：エントリを `main.android.tsx` に
// し、DOM/HTML を伴わない単一の IIFE として出力する。生成物（`dist-android/tsubame.js`）を
// Torimi Android ホストがネットワーク fetch して Hermes へロードする。JSX 変換はブラウザ用
// `vite.config.ts` と同じ automatic runtime（`jsxImportSource` → `@tsubame/react`）。
export default defineConfig({
  esbuild: {
    jsx: 'automatic',
    jsxImportSource: '@tsubame/react',
  },
  // React は `process.env.NODE_ENV` で prod/dev エントリを分岐する。Hermes に `process` は
  // 無いため、未置換だと eval 時に `ReferenceError` で落ちる（`vite.config.torimi.ts` と
  // 同じ理由・同じ対処）。
  define: {
    'process.env.NODE_ENV': JSON.stringify('production'),
  },
  build: {
    // Hermes は匿名 class 式（`var X = class {}`）を正しく評価できないため、ビルド後に
    // 共有スクリプト（`Tsubame/scripts/lower-for-hermes.mjs`）で class/modern 構文を ES5
    // 相当へ降格する（build:android スクリプト, ADR-0112）。vite の target はここでは通常値。
    target: 'es2020',
    outDir: 'dist-android',
    emptyOutDir: true,
    cssCodeSplit: false,
    // デバッグしやすさ優先で非圧縮。サイズ最適化は後段（hermesc/リリース）で。
    minify: false,
    lib: {
      entry: fileURLToPath(new URL('./src/main.android.tsx', import.meta.url)),
      formats: ['iife'],
      name: 'TsubameReactTodoAndroid',
      fileName: () => 'tsubame.js',
    },
  },
});
