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
});
