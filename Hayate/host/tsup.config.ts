import { defineConfig } from 'tsup';

export default defineConfig({
  // Android（埋め込み Hermes, ADR-0112）は WASM を巻き込まないよう `./native` を
  // 独立エントリにする。`index.ts` だけが `hayate-adapter-web*` を動的 import する。
  entry: ['src/index.ts', 'src/native.ts'],
  format: ['esm'],
  dts: true,
  clean: true,
  sourcemap: true,
  target: 'es2022',
  // CanvasKit's JS loader and its WASM binary must be included in the published Host package;
  // leaving the dependency external would make `locateFile` point into a consumer's node_modules.
  noExternal: ['canvaskit-wasm'],
  loader: { '.wasm': 'file' },
});
