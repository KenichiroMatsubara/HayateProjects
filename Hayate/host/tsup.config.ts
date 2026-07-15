import { defineConfig } from 'tsup';

export default defineConfig({
  // Android（埋め込み Hermes, ADR-0112）は WASM を巻き込まないよう `./native` を
  // 独立エントリにする。`index.ts` だけが `hayate-adapter-web*` を動的 import する。
  entry: ['src/index.ts', 'src/native.ts', 'src/renderer-policy.ts'],
  format: ['esm'],
  dts: true,
  clean: true,
  sourcemap: true,
  target: 'es2022',
  // CanvasKit は dependency のまま consumer bundler に渡す。ここで JS/WASM を Host に
  // 焼き込むと `.wasm?url` が単なる相対文字列に変わり、後段の Vite が asset dependency
  // として追跡できない。最終アプリの bundler に解決させることで、その base / assetsDir
  // に対応した URL と WASM 配信物を同時に生成させる。
  external: ['canvaskit-wasm', 'canvaskit-wasm/*'],
});
