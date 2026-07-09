import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vite';
import solid from 'vite-plugin-solid';

// Torimi App Bundle（ADR-0001 のスライス #1）向けの単一ファイルバンドル。
//
// ブラウザ用 `vite.config.ts` と同じ solid-js/universal 変換を使いつつ、エントリを
// 全ターゲット共通の `main.bundle.tsx`（#767）にし、DOM/HTML を伴わない単一の IIFE として出力する。生成物
// （`dist-torimi/bundle.js`）を Torimi dev-server が HTTP 配信し、Web ホストが
// fetch → eval して `globalThis.__torimiMount` を拾う。
//
// android（Hermes）と違いブラウザの eval で実行するため、class/modern 構文の降格は不要。
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
    target: 'es2020',
    outDir: 'dist-torimi',
    emptyOutDir: true,
    cssCodeSplit: false,
    // デバッグしやすさ優先で非圧縮。
    minify: false,
    lib: {
      entry: fileURLToPath(new URL('./src/main.bundle.tsx', import.meta.url)),
      formats: ['iife'],
      name: 'TsubameTodoTorimi',
      fileName: () => 'bundle.js',
    },
  },
});
