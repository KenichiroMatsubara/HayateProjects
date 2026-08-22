import { defineConfig } from 'vite';

// react-demo と同型。JSX は React 標準の automatic runtime で変換し、import 先だけ
// `@torimi/tsubame-react/jsx-runtime` に向け替える（`jsxImportSource`）。compile プラグイン不要（ADR-0010）。
export default defineConfig({
  // GitHub Pages の project site（/HayateProjects/shogi/ 配下）へ置く場合に base を合わせる。
  base: process.env.VITE_BASE ?? '/',
  esbuild: {
    jsx: 'automatic',
    jsxImportSource: '@torimi/tsubame-react',
  },
});
