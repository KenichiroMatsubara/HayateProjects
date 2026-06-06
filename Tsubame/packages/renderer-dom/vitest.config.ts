import { defineConfig } from 'vitest/config';

export default defineConfig({
  test: {
    environment: 'happy-dom',
    exclude: ['test/**', '**/node_modules/**'],
    server: {
      deps: {
        inline: [
          '@tsubame/protocol-generated',
          '@tsubame/hayate-css-catalog',
        ],
      },
    },
  },
});
