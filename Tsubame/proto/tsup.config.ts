import { copyFileSync } from 'node:fs';

import { defineConfig } from 'tsup';

// protocol-generated ships a dist build so external consumers don't need to
// compile the generated .ts themselves (ADR-0007 §5). Internal workspace
// consumers keep resolving the source `./generated/*.ts` via the dev `exports`;
// `publishConfig.exports` swaps in these dist entries only at publish time.
// The wire constants live in catalog.json (a data asset, not a module), so it
// is copied verbatim into dist next to the compiled entries.
export default defineConfig({
  entry: [
    'generated/index.ts',
    'generated/protocol.ts',
    'generated/codec.ts',
    'generated/catalog.ts',
    'generated/delivery.ts',
    'generated/recorder.ts',
  ],
  format: ['esm'],
  dts: true,
  clean: true,
  sourcemap: true,
  target: 'es2022',
  onSuccess: async () => {
    copyFileSync('generated/catalog.json', 'dist/catalog.json');
  },
});
