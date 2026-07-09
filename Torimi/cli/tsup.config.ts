import { defineConfig } from 'tsup';

// Two entries: the `torimi` bin (cli.ts, keeps its shebang) and a small library
// surface (index.ts) so the pure helpers can be imported/tested. @babel/* and
// @torimi/dev-server stay external (declared deps), never bundled in.
export default defineConfig({
  entry: ['src/cli.ts', 'src/index.ts'],
  format: ['esm'],
  dts: true,
  clean: true,
  sourcemap: true,
  target: 'es2022',
  external: ['@babel/core', '@babel/preset-env', '@torimi/dev-server'],
});
