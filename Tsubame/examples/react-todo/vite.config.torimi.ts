import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';

// Torimi react App Bundle（#531：FW 非依存の実証）向けの単一ファイルバンドル。
//
// ブラウザ用 `vite.config.ts` と同じ React automatic runtime（`jsxImportSource` を
// `@tsubame/react` に向ける）を使いつつ、エントリを `main.torimi.tsx` にし、DOM/HTML を
// 伴わない単一の IIFE として出力する。生成物（`dist-torimi/bundle.js`）を Torimi
// dev-server が HTTP 配信し、Web ホストが fetch → eval して `globalThis.__torimiMount` を拾う。
//
// solid 版（`examples/todo/vite.config.torimi.ts`）と対称：FW 固有の変換はここ（バンドル側）
// に閉じ、出力する wire シームは同一なので同じホストが描画できる（ADR-0001）。ブラウザの eval で
// 実行するため class/modern 構文の降格は不要。
export default defineConfig({
  esbuild: {
    jsx: 'automatic',
    jsxImportSource: '@tsubame/react',
  },
  // React は `process.env.NODE_ENV` で prod/dev エントリを分岐する。バンドルはブラウザ host の
  // `eval` で実行され `process` が無いため、未置換だと `ReferenceError: process is not defined` で
  // 起動が落ちる（solid 版は `process` を参照しないので顕在化しない）。production 実体へ静的置換し、
  // 残る `process` 参照は `typeof process === 'object'` ガード下のみ（ブラウザでは未到達）にする。
  define: {
    'process.env.NODE_ENV': JSON.stringify('production'),
  },
  build: {
    target: 'es2020',
    outDir: 'dist-torimi',
    emptyOutDir: true,
    cssCodeSplit: false,
    // デバッグしやすさ優先で非圧縮。
    minify: false,
    lib: {
      entry: fileURLToPath(new URL('./src/main.torimi.tsx', import.meta.url)),
      formats: ['iife'],
      name: 'TsubameReactTodoTorimi',
      fileName: () => 'bundle.js',
    },
  },
});
