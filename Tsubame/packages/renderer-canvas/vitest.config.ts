import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    projects: [
      {
        extends: true,
        test: {
          name: 'unit',
          environment: 'node',
          include: ['src/**/*.test.ts'],
          exclude: [
            'src/wasm-integration.test.ts',
            'src/golden-frame.test.ts',
            'src/golden-frame-parity.test.ts',
            'test/**',
            '**/node_modules/**',
          ],
          server: {
            deps: {
              inline: [
                '@tsubame/protocol-generated',
                '@tsubame/hayate-css-catalog',
                '@tsubame/renderer-dom',
              ],
            },
          },
        },
      },
      {
        extends: true,
        test: {
          name: 'wasm',
          environment: 'happy-dom',
          include: [
            'src/wasm-integration.test.ts',
            'src/golden-frame.test.ts',
            'src/golden-frame-parity.test.ts',
          ],
          server: {
            deps: {
              inline: [
                '@tsubame/protocol-generated',
                'hayate-adapter-web-null',
                '@tsubame/solid',
              ],
            },
          },
        },
      },
    ],
  },
});
