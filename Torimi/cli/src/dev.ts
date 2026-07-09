import { watch } from 'node:fs';
import { join } from 'node:path';

import { ALL_INTERFACES_HOSTNAME, createBundleDevServer, printStartupBanner } from '@torimi/dev-server';

import { buildForTarget } from './build.js';
import type { TorimiConfig } from './config.js';
import type { Target } from './constants.js';
import { REBUILD_DEBOUNCE_MS, portForTarget } from './constants.js';
import { reclaimStaleDevServer, removePidFile, waitForPortFree, writePidFile } from './port.js';

export interface DevOptions {
  /** ポート上書き（既定はターゲット既定ポート、env TORIMI_DEV_PORT で渡す）。 */
  readonly port?: number;
}

// `torimi dev [target]` の full reload ループ（ADR-0008 §1/§3）:
//   ソース変更 → 再ビルド（native はさらに降格）→ 降格済みの別パスを配信 →
//   @torimi/dev-server が接続中ホストへ WS reload。配信・reload・QR は下層 dev-server がそのまま担う。
export async function dev(config: TorimiConfig, target: Target, cwd: string, options: DevOptions = {}): Promise<void> {
  const port = options.port ?? portForTarget(target);

  // 同時に複数ビルドを走らせない。実行中に来た変更は「次の 1 回」だけ予約して合体する。
  let building = false;
  let queued = false;
  let debounce: ReturnType<typeof setTimeout> | undefined;
  let servedPath = '';

  async function rebuild(): Promise<void> {
    if (building) {
      queued = true;
      return;
    }
    building = true;
    do {
      queued = false;
      try {
        // 降格まで含む buildForTarget を 1 ステップで完走してから servedPath を差し替える。
        // dev-server はこの降格済みファイルだけを watch するので中途半端な未降格は配られない。
        servedPath = await buildForTarget(config, target, cwd);
      } catch (err) {
        // ビルド失敗は致命ではない（直して保存すれば次の watch で再試行）。
        console.error(`torimi dev: build failed (${String(err)}) — save again to retry`);
      }
    } while (queued);
    building = false;
  }

  function scheduleRebuild(): void {
    if (debounce != null) clearTimeout(debounce);
    debounce = setTimeout(() => {
      debounce = undefined;
      void rebuild();
    }, REBUILD_DEBOUNCE_MS);
  }

  console.log(`torimi dev (${target}): initial build…`);
  await rebuild();
  if (!servedPath) throw new Error('torimi dev: initial build failed — cannot start dev server');

  const watchDir = join(cwd, config.watch);
  const watcher = watch(watchDir, { recursive: true }, () => scheduleRebuild());

  // 前回の残骸（自分自身）がポートを掴んだままなら片付けて解放を待つ。無関係なプロセスには触らない。
  reclaimStaleDevServer(cwd, port);
  await waitForPortFree(port);

  // 0.0.0.0 で listen して同じ LAN の端末からも到達可能に（dev-only）。起動バナーが LAN URL と
  // QR を出すので端末のカメラで読んで host へ入力できる。
  const server = createBundleDevServer({ bundlePath: servedPath, port, hostname: ALL_INTERFACES_HOSTNAME });
  try {
    await server.listen();
  } catch (err) {
    const code = (err as { code?: string })?.code ?? err;
    console.error(`torimi dev: could not listen on port ${port} (${code}).`);
    console.error('  Another process may be using this port (override with TORIMI_DEV_PORT).');
    watcher.close();
    process.exit(1);
  }
  const pidFile = writePidFile(cwd, port);
  printStartupBanner({ port, loopbackUrl: `http://localhost:${port}` });

  const shutdown = (): void => {
    if (debounce != null) clearTimeout(debounce);
    watcher.close();
    removePidFile(pidFile);
    server.close().finally(() => process.exit(0));
  };
  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);
}
