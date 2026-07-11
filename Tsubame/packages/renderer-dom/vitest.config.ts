import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'happy-dom',
    exclude: ['test/**', '**/node_modules/**'],
    server: {
      deps: {
        inline: [
          '@torimi/tsubame-protocol-generated',
          '@torimi/tsubame-hayate-css-catalog',
        ],
      },
    },
  },
});
