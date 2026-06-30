// Miharashi 最小 dev server の起動ラッパ（e2e / ローカル用・#531）。
//
// solid 版（examples/todo/scripts/miharashi-dev-server.mjs）と同型。full reload ループ
// （ADR-0001）を端から端まで繋ぐ：
//   1. vite の build watch で main.miharashi.tsx をソース変更ごとに再ビルドし、単一 App Bundle
//      （dist-miharashi/bundle.js）を更新し続ける（ビルドは外部の責務）。
//   2. `@miharashi/dev-server` がその bundle を HTTP 配信し、bundle の更新を watch して
//      接続中のホストに WS で `reload` を送る（FW/ビルドツール非依存 — react でも solid でも同じ）。
//
// ビルドは vite の Node API（`build`）で**この**プロセス内に回す。`pnpm exec vite` を子プロセス
// として spawn しないので、シェルにも OS にも依存しない（Windows の `pnpm.cmd` を Node の spawn が
// 解決できず ENOENT/EINVAL になる問題が原理的に起きない）。
//
// 初回ビルドの完了は dev-server の `/bundle.js` が 200 を返すこと（playwright の webServer
// readiness 判定）で待つ — それまでは 404 を返す。
import { fileURLToPath } from 'node:url';
import { build } from 'vite';
import {
  ALL_INTERFACES_HOSTNAME,
  createBundleDevServer,
  printStartupBanner,
} from '@miharashi/dev-server';

// `playwright.config.ts` の MIHARASHI_DEV_PORT と一致させる既定ポート（solid 版 5181 と衝突回避）。
const DEFAULT_PORT = 5183;

const exampleRoot = fileURLToPath(new URL('..', import.meta.url));
const configFile = fileURLToPath(new URL('../vite.config.miharashi.ts', import.meta.url));
const bundlePath = fileURLToPath(new URL('../dist-miharashi/bundle.js', import.meta.url));
const port = Number(process.env.MIHARASHI_DEV_PORT ?? DEFAULT_PORT);

// ソースを watch して bundle を更新し続けるビルド（FW 固有 = バンドル側の責務）。watch を有効に
// すると `build` は RollupWatcher を返してブロックしない（in-process でビルドし続ける）。
const watcher = await build({
  root: exampleRoot,
  configFile,
  build: { watch: {} },
});

// 0.0.0.0 で listen して同じ LAN のスマホ／別端末からも到達できるようにする（dev-only ツール）。
// 起動コマンドは LAN URL とその QR を出すので、スマホのカメラで読み取って host へ入力できる。
const server = createBundleDevServer({ bundlePath, port, hostname: ALL_INTERFACES_HOSTNAME });
await server.listen();
printStartupBanner({ port, loopbackUrl: `http://localhost:${port}` });

// プロセス終了時に watcher も確実に閉じる。
const shutdown = () => {
  Promise.resolve(watcher.close?.())
    .then(() => server.close())
    .finally(() => process.exit(0));
};
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
