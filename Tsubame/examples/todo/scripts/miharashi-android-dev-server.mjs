// Miharashi の Android 向け dev server 起動ラッパ（ローカル実機 / エミュレータ用）。
//
// Android ホスト（事前ビルド済みネイティブシェル）は dev-server から `/bundle.js` を
// 実行時 fetch し、埋め込み Hermes で eval する（bundle_source.rs / Miharashi CONTEXT.md）。
// そのため配信する App Bundle は **Hermes 用に降格済み**（main.android.tsx → vite build →
// lower-for-hermes）でなければならない。Web ホスト用の `miharashi-dev-server.mjs`（es2020 の
// main.miharashi.tsx）とは配信物が異なる点が要点。
//
// full reload ループ（Miharashi ADR-0001）を端から端まで繋ぐ：
//   1. ソース（src/）を watch し、変更ごとに `build:android`（vite build + Hermes 降格）を
//      回して単一 App Bundle（dist-android/tsubame.js）を更新し続ける。ビルドは外部の責務。
//   2. `@miharashi/dev-server` がその bundle を HTTP 配信し、bundle の更新を watch して
//      接続中のホスト（端末）に WS で `reload` を送る（FW/ビルドツール非依存）。降格後の
//      最終成果物だけが書かれるよう、降格まで含む build:android を 1 ステップで回してから
//      dev-server の file watch が発火する（中途半端な未降格 bundle を配らない）。
//
// 既定ポートはネイティブ既定（dev_server_target.rs の DEFAULT_DEV_SERVER_PORT=5179）に合わせる。
// 端末 UI で URL 未入力のエミュレータは 10.0.2.2:5179 へ落ちるので、このポートなら無入力で繋がる。
import { spawn } from 'node:child_process';
import { watch } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createBundleDevServer } from '@miharashi/dev-server';

// ネイティブ既定（dev_server_target.rs DEFAULT_DEV_SERVER_PORT）と一致させる既定ポート。
const DEFAULT_PORT = 5179;
// 連続したファイル書き込み（保存・エディタの一時ファイル等）を 1 回の再ビルドにまとめる猶予。
const REBUILD_DEBOUNCE_MS = 120;

const todoRoot = fileURLToPath(new URL('..', import.meta.url));
const srcDir = fileURLToPath(new URL('../src', import.meta.url));
const bundlePath = fileURLToPath(new URL('../dist-android/tsubame.js', import.meta.url));
const port = Number(process.env.MIHARASHI_DEV_PORT ?? DEFAULT_PORT);

// 同時に複数の build を走らせない。実行中に来た変更は「次の 1 回」だけ予約する（合体）。
let building = false;
let queued = false;
let debounceTimer;

/** `build:android`（vite build + Hermes 降格）を完走させる。成功で dist-android/tsubame.js を更新。 */
function runBuild() {
  return new Promise((resolve) => {
    const child = spawn('pnpm', ['run', 'build:android'], {
      cwd: todoRoot,
      stdio: 'inherit',
    });
    child.on('exit', (code) => {
      if (code !== 0) {
        // ビルド失敗は致命ではない（直してまた保存すれば次の watch で再ビルドされる）。
        console.error(`Miharashi android dev: build:android が exit ${code} で失敗しました（保存で再試行）`);
      }
      resolve();
    });
  });
}

/** 実行中なら次の 1 回を予約し、空いていれば即ビルド。終わってから予約があれば続けて回す。 */
async function rebuild() {
  if (building) {
    queued = true;
    return;
  }
  building = true;
  do {
    queued = false;
    await runBuild();
  } while (queued);
  building = false;
}

/** debounce 付きで rebuild をトリガする（保存連打や一時ファイルの連続イベントを 1 回に畳む）。 */
function scheduleRebuild() {
  if (debounceTimer != null) clearTimeout(debounceTimer);
  debounceTimer = setTimeout(() => {
    debounceTimer = undefined;
    void rebuild();
  }, REBUILD_DEBOUNCE_MS);
}

// 初回ビルド（dist-android/tsubame.js を最初に用意する）。完了を待ってから watch を張る。
console.log('Miharashi android dev: 初回ビルド中（build:android）…');
await rebuild();

// ソース変更で再ビルド。recursive watch は Node 20+ で Linux/macOS/Windows いずれも対応。
const srcWatcher = watch(srcDir, { recursive: true }, () => scheduleRebuild());

// dist-android/tsubame.js を watch し、降格まで終わった最終 bundle の更新ごとに WS reload を送る。
const server = createBundleDevServer({ bundlePath, port });
const origin = await server.listen();
console.log(`Miharashi android dev server: ${origin}`);
console.log('  端末 / エミュレータの dev-server URL にこの host:port を入力してください。');
console.log('  エミュレータで URL 未入力なら 10.0.2.2:' + port + ' に落ちます（既定）。');

// プロセス終了時に watcher と server を確実に閉じる。
const shutdown = () => {
  if (debounceTimer != null) clearTimeout(debounceTimer);
  srcWatcher.close();
  server.close().finally(() => process.exit(0));
};
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
