import { defineConfig } from 'vite';

// tsubame-react は react-reconciler ベースの Adapter。JSX は React 標準の automatic
// runtime で変換し、import 先だけ `@tsubame/react/jsx-runtime` に向け替える
// （`jsxImportSource`）。compile プラグインは不要（ADR-0010）。
export default defineConfig({
  // GitHub Pages の project site（/HayateProjects/ 配下）へ置く場合に base を合わせる。
  // ローカル dev/build には影響させず、Pages デプロイ workflow が VITE_BASE を渡す。
  base: process.env.VITE_BASE ?? '/',
  esbuild: {
    jsx: 'automatic',
    jsxImportSource: '@tsubame/react',
  },
});
