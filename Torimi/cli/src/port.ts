import { execFileSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { connect } from 'node:net';
import { dirname, join } from 'node:path';

// 前回異常終了などでポートを掴んだまま残った「自分自身の残骸」だけを回収する既存の安全策を
// 引き継ぐ（ADR-0008 §2）。共有バイナリ化で ps コマンドライン一致だけでは自プロジェクトを
// 見分けられないため、プロジェクト+ポート単位の pidfile に自 PID を残し、次回起動時にその
// PID が「生きている torimi プロセス」であるときだけ SIGTERM する。無関係なプロセスには
// 一切触らない（片付かなければ通常どおり EADDRINUSE で失敗させる）。
export function pidFilePath(cwd: string, port: number): string {
  return join(cwd, 'node_modules', '.cache', 'torimi', `dev-${port}.pid`);
}

function processCommand(pid: number): string | null {
  try {
    return execFileSync('ps', ['-o', 'command=', '-p', String(pid)], { encoding: 'utf8' });
  } catch {
    return null; // ps が無い / プロセスが既に居ない
  }
}

export function reclaimStaleDevServer(cwd: string, port: number): void {
  const file = pidFilePath(cwd, port);
  if (!existsSync(file)) return;
  let pid: number;
  try {
    pid = Number(readFileSync(file, 'utf8').trim());
  } catch {
    return;
  }
  if (!Number.isInteger(pid) || pid <= 0 || pid === process.pid) return;
  const cmd = processCommand(pid);
  if (!cmd || !/torimi/.test(cmd)) return; // 既に居ない、または PID が無関係なプロセスに再利用されている
  try {
    process.kill(pid, 'SIGTERM');
    console.log(`torimi dev: reclaimed a stale dev server (PID ${pid}) holding port ${port}.`);
  } catch {
    // 既に終了済みなど
  }
}

export function writePidFile(cwd: string, port: number): string {
  const file = pidFilePath(cwd, port);
  mkdirSync(dirname(file), { recursive: true });
  writeFileSync(file, String(process.pid));
  return file;
}

export function removePidFile(file: string): void {
  try {
    rmSync(file);
  } catch {
    // 既に無い
  }
}

/** ポートに何か listen 中か probe する（TCP connect が繋がれば in-use）。 */
export function isPortInUse(port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = connect({ port, host: '127.0.0.1' });
    socket.once('connect', () => {
      socket.destroy();
      resolve(true);
    });
    socket.once('error', () => resolve(false));
  });
}

/** SIGTERM 後に OS がソケットを解放するまでの一瞬のラグを吸収して、ポートが空くまで待つ。 */
export async function waitForPortFree(port: number, attempts = 10, delayMs = 200): Promise<void> {
  for (let i = 0; i < attempts; i += 1) {
    if (!(await isPortInUse(port))) return;
    await new Promise((r) => setTimeout(r, delayMs));
  }
}
