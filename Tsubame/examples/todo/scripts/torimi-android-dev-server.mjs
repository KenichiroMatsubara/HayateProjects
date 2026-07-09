// Torimi の Android 向け dev server 起動ラッパ（ローカル実機 / エミュレータ用）。
//
// Android ホスト（事前ビルド済みネイティブシェル）は dev-server から `/bundle.js` を
// 実行時 fetch し、埋め込み Hermes で eval する（bundle_source.rs / Torimi CONTEXT.md）。
// そのため配信する App Bundle は **Hermes 用に降格済み**（main.bundle.tsx → vite build →
// lower-for-hermes）でなければならない。Web ホスト用の `torimi-dev-server.mjs`（es2020 の
// main.bundle.tsx）とは配信物が異なる点が要点。
//
// full reload ループ（Torimi ADR-0001）を端から端まで繋ぐ：
//   1. ソース（src/）を watch し、変更ごとに `torimi:native:build`（vite build + Hermes 降格）を
//      回して単一 App Bundle（dist-android/tsubame.js）を更新し続ける。ビルドは外部の責務。
//   2. `@torimi/dev-server` がその bundle を HTTP 配信し、bundle の更新を watch して
//      接続中のホスト（端末）に WS で `reload` を送る（FW/ビルドツール非依存）。降格後の
//      最終成果物だけが書かれるよう、降格まで含む torimi:native:build を 1 ステップで回してから
//      dev-server の file watch が発火する（中途半端な未降格 bundle を配らない）。
//
// 既定ポートはネイティブ既定（dev_server_target.rs の DEFAULT_DEV_SERVER_PORT=5179）に合わせる。
// 端末 UI で URL 未入力のエミュレータは 10.0.2.2:5179 へ落ちるので、このポートなら無入力で繋がる。
import { execFileSync, spawn } from 'node:child_process';
import { watch } from 'node:fs';
import { connect } from 'node:net';
import { fileURLToPath } from 'node:url';
import {
  ALL_INTERFACES_HOSTNAME,
  createBundleDevServer,
  printStartupBanner,
} from '@torimi/dev-server';

// このスクリプト自身の目印（`ps` のコマンドラインに現れるパス片）。
// 前回異常終了などでポートを掴んだまま残った「自分自身の残骸」だけを見分けるのに使う。
const SELF_SIGNATURE = 'torimi-android-dev-server.mjs';

/**
 * 指定ポートを掴んでいる残留プロセスのうち、**このスクリプト自身の前回起動**だと確認できた
 * ものだけを終了させる。無関係な Node プロセス（他のアプリの dev server 等）が同じポートを
 * 使っている場合は何もせず、通常どおり EADDRINUSE で失敗させる（安全側に倒す）。
 */
function reclaimStalePort(targetPort) {
  let pids;
  try {
    const out = execFileSync('lsof', ['-t', '-i', `tcp:${targetPort}`, '-sTCP:LISTEN'], {
      encoding: 'utf8',
    });
    pids = out
      .split('\n')
      .map((s) => s.trim())
      .filter(Boolean)
      .map(Number)
      .filter((pid) => pid !== process.pid);
  } catch {
    return; // lsof が無い / 掴んでいるプロセスが無い — 何もしない
  }

  for (const pid of pids) {
    let cmdline;
    try {
      cmdline = execFileSync('ps', ['-o', 'command=', '-p', String(pid)], { encoding: 'utf8' });
    } catch {
      continue; // 調べているうちに既に終了した等
    }
    if (!cmdline.includes(SELF_SIGNATURE)) continue; // 無関係なプロセスには一切触らない

    try {
      process.kill(pid, 'SIGTERM');
      console.log(
        `Torimi android dev: ポート ${targetPort} を掴んでいた前回の残留プロセス（PID ${pid}）を終了しました。`,
      );
    } catch {
      // 既に終了済みなど — 無視
    }
  }
}

/** ポートに何か listen 中か probe する（TCP connect が繋がれば in-use）。 */
function isPortInUse(targetPort) {
  return new Promise((resolve) => {
    const socket = connect({ port: targetPort, host: '127.0.0.1' });
    socket.once('connect', () => {
      socket.destroy();
      resolve(true);
    });
    socket.once('error', () => resolve(false));
  });
}

/**
 * ポートが空くまで少し待つ。`reclaimStalePort` が SIGTERM を送っても OS がソケットを解放する
 * まで一瞬ラグがあるため。無関係なプロセスが掴んでいて空かない場合はタイムアウトしてそのまま
 * `server.listen()` に委ね、通常どおり EADDRINUSE で失敗させる（同じ server インスタンスへの
 * listen() 再試行は net.Server の想定外の使い方でエラーリスナーが積み上がるため避ける）。
 */
async function waitForPortFree(targetPort, attempts = 10, delayMs = 200) {
  for (let i = 0; i < attempts; i += 1) {
    if (!(await isPortInUse(targetPort))) return;
    await new Promise((r) => setTimeout(r, delayMs));
  }
}

// ネイティブ既定（dev_server_target.rs DEFAULT_DEV_SERVER_PORT）と一致させる既定ポート。
const DEFAULT_PORT = 5179;
// 連続したファイル書き込み（保存・エディタの一時ファイル等）を 1 回の再ビルドにまとめる猶予。
const REBUILD_DEBOUNCE_MS = 120;

const todoRoot = fileURLToPath(new URL('..', import.meta.url));
const srcDir = fileURLToPath(new URL('../src', import.meta.url));
const bundlePath = fileURLToPath(new URL('../dist-android/tsubame.js', import.meta.url));
const port = Number(process.env.TORIMI_DEV_PORT ?? DEFAULT_PORT);

// 同時に複数の build を走らせない。実行中に来た変更は「次の 1 回」だけ予約する（合体）。
let building = false;
let queued = false;
let debounceTimer;

/** `torimi:native:build`（vite build + Hermes 降格）を完走させる。成功で dist-android/tsubame.js を更新。 */
function runBuild() {
  return new Promise((resolve) => {
    const child = spawn('pnpm', ['run', 'torimi:native:build'], {
      cwd: todoRoot,
      stdio: 'inherit',
    });
    child.on('exit', (code) => {
      if (code !== 0) {
        // ビルド失敗は致命ではない（直してまた保存すれば次の watch で再ビルドされる）。
        console.error(`Torimi android dev: torimi:native:build が exit ${code} で失敗しました（保存で再試行）`);
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
console.log('Torimi android dev: 初回ビルド中（torimi:native:build）…');
await rebuild();

// ソース変更で再ビルド。recursive watch は Node 20+ で Linux/macOS/Windows いずれも対応。
const srcWatcher = watch(srcDir, { recursive: true }, () => scheduleRebuild());

// 前回起動の残骸（自分自身）がポートを掴んだままなら先に片付け、解放されるのを少し待つ。
// 無関係なプロセスには触らない（片付かなければ待つだけ待って下の listen() を通常どおり失敗させる）。
reclaimStalePort(port);
await waitForPortFree(port);

// dist-android/tsubame.js を watch し、降格まで終わった最終 bundle の更新ごとに WS reload を送る。
// 0.0.0.0 で listen して実機（同じ Wi‑Fi の LAN）からも到達できるようにする。起動コマンドは
// LAN URL と QR を出すので、端末の「QR スキャン」ボタンのカメラで読めばそのまま接続できる。
const server = createBundleDevServer({ bundlePath, port, hostname: ALL_INTERFACES_HOSTNAME });
try {
  await server.listen();
} catch (err) {
  console.error(`Torimi android dev: ポート ${port} で listen できませんでした（${err?.code ?? err}）。`);
  console.error('  別プロセスがこのポートを使っていないか確認してください（TORIMI_DEV_PORT で変更可）。');
  process.exit(1);
}
printStartupBanner({ port, loopbackUrl: `http://localhost:${port}` });
console.log('  実機: 上の QR を端末アプリの「QR スキャン」で読むか、LAN URL を入力してください。');
console.log('  エミュレータで URL 未入力なら 10.0.2.2:' + port + ' に落ちます（既定）。');

// プロセス終了時に watcher と server を確実に閉じる。
const shutdown = () => {
  if (debounceTimer != null) clearTimeout(debounceTimer);
  srcWatcher.close();
  server.close().finally(() => process.exit(0));
};
process.on('SIGINT', shutdown);
process.on('SIGTERM', shutdown);
