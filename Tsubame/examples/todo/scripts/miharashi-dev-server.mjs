// Miharashi 最小 dev server の起動ラッパ（e2e / ローカル用）。
//
// full reload ループ（ADR-0001）を端から端まで繋ぐ：
//   1. `vite build --watch` で main.miharashi.tsx をソース変更ごとに再ビルドし、単一 App Bundle
//      （dist-miharashi/bundle.js）を更新し続ける（ビルドは外部の責務）。
//   2. `@miharashi/dev-server` がその bundle を HTTP 配信し、bundle の更新を watch して
//      接続中のホストに WS で `reload` を送る（FW/ビルドツール非依存）。
//
// 初回ビルドの完了は dev-server の `/bundle.js` が 200 を返すこと（playwright の webServer
// readiness 判定）で待つ — それまでは 404 を返す。
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createBundleDevServer } from '@miharashi/dev-server';

// `playwright.config.ts` の MIHARASHI_DEV_PORT と一致させる既定ポート。
const DEFAULT_PORT = 5181;

const todoRoot = fileURLToPath(new URL('..', import.meta.url));
const bundlePath = fileURLToPath(new URL('../dist-miharashi/bundle.js', import.meta.url));
const port = Number(process.env.MIHARASHI_DEV_PORT ?? DEFAULT_PORT);

// ソースを watch して bundle を更新し続けるビルドプロセス（FW 固有 = バンドル側の責務）。
const builder = spawn(
  'pnpm',
  ['exec', 'vite', 'build', '--config', 'vite.config.miharashi.ts', '--watch'],
  { cwd: todoRoot, stdio: 'inherit' },
);

const server = createBundleDevServer({ bundlePath, port });
const origin = await server.listen();
console.log(`Miharashi dev server: ${origin}`);

// プロセス終了時に build --watch の子も確実に落とす。
const shutdown = () => {
  builder.kill();
  server.close().finally(() => process.exit(0));
};
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
process.on('exit', () => builder.kill());
