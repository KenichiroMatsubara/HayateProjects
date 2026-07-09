import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts', 'src/vite.ts'],
  format: ['esm'],
  dts: true,
  clean: true,
  sourcemap: true,
  target: 'es2022',
  external: ['solid-js', 'solid-js/universal', 'vite', 'vite-plugin-solid'],
});
