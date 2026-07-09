import { defineConfig } from 'tsup';

// The `create-torimi` bin (cli.ts, keeps its shebang) plus a small library surface
// (index.ts) exposing the pure scaffold helpers for tests and bake-template.mjs.
// The template itself is copied into dist/template by bake-template.mjs after this.
export default defineConfig({
  entry: ['src/cli.ts', 'src/index.ts'],
  format: ['esm'],
  dts: true,
  clean: true,
  sourcemap: true,
  target: 'es2022',
});
