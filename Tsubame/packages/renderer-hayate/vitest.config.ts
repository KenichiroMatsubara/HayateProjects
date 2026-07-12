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
                '@torimi/tsubame-protocol-generated',
                '@torimi/tsubame-hayate-css-catalog',
                '@torimi/tsubame-renderer-dom',
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
                '@torimi/tsubame-protocol-generated',
                'hayate-adapter-web-null',
                '@torimi/tsubame-solid',
              ],
            },
          },
        },
      },
    ],
  },
});
