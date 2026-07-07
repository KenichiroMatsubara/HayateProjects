// Torimi 最小 dev server の起動ラッパ（e2e / ローカル用）。
//
// full reload ループ（ADR-0001）を端から端まで繋ぐ：
//   1. `vite build --watch` で main.torimi.tsx をソース変更ごとに再ビルドし、単一 App Bundle
//      （dist-torimi/bundle.js）を更新し続ける（ビルドは外部の責務）。
//   2. `@torimi/dev-server` がその bundle を HTTP 配信し、bundle の更新を watch して
//      接続中のホストに WS で `reload` を送る（FW/ビルドツール非依存）。
//
// 初回ビルドの完了は dev-server の `/bundle.js` が 200 を返すこと（playwright の webServer
// readiness 判定）で待つ — それまでは 404 を返す。
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import {
  ALL_INTERFACES_HOSTNAME,
  createBundleDevServer,
  printStartupBanner,
} from '@torimi/dev-server';

// `playwright.config.ts` の TORIMI_DEV_PORT と一致させる既定ポート。
const DEFAULT_PORT = 5181;

const todoRoot = fileURLToPath(new URL('..', import.meta.url));
const bundlePath = fileURLToPath(new URL('../dist-torimi/bundle.js', import.meta.url));
const port = Number(process.env.TORIMI_DEV_PORT ?? DEFAULT_PORT);

// ソースを watch して bundle を更新し続けるビルドプロセス（FW 固有 = バンドル側の責務）。
const builder = spawn(
  'pnpm',
  ['exec', 'vite', 'build', '--config', 'vite.config.torimi.ts', '--watch'],
  { cwd: todoRoot, stdio: 'inherit' },
);

// 0.0.0.0 で listen して同じ LAN のスマホ／別端末からも到達できるようにする（dev-only ツール）。
// 起動コマンドは LAN URL とその QR を出すので、スマホのカメラで読み取って host へ入力できる。
const server = createBundleDevServer({ bundlePath, port, hostname: ALL_INTERFACES_HOSTNAME });
await server.listen();
printStartupBanner({ port, loopbackUrl: `http://localhost:${port}` });

// プロセス終了時に build --watch の子も確実に落とす。
const shutdown = () => {
  builder.kill();
  server.close().finally(() => process.exit(0));
};
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
process.on('exit', () => builder.kill());
