// Miharashi 最小 dev server の起動ラッパ（e2e / ローカル用）。
// ビルド済みの単一 App Bundle（dist-miharashi/bundle.js）を HTTP 配信するだけ。
import { fileURLToPath } from 'node:url';
import { createBundleDevServer } from '@miharashi/dev-server';

// `playwright.config.ts` の MIHARASHI_DEV_PORT と一致させる既定ポート。
const DEFAULT_PORT = 5181;

const bundlePath = fileURLToPath(new URL('../dist-miharashi/bundle.js', import.meta.url));
const port = Number(process.env.MIHARASHI_DEV_PORT ?? DEFAULT_PORT);

const server = createBundleDevServer({ bundlePath, port });
const origin = await server.listen();
console.log(`Miharashi dev server: ${origin}`);
