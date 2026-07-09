import { spawn } from 'node:child_process';
import { join } from 'node:path';

import type { TorimiConfig } from './config.js';
import type { Target } from './constants.js';
import { loweredBundlePath } from './constants.js';
import { lowerFileTo } from './lower.js';

// config.build を不透明にシェル実行する。CLI は FW もビルドツールも解さない — build は設定値。
export function runShell(command: string, cwd: string): Promise<void> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, { cwd, stdio: 'inherit', shell: true });
    child.on('exit', (code) => {
      if (code === 0) resolve();
      else reject(new Error(`torimi: build command failed (exit ${code}): ${command}`));
    });
    child.on('error', reject);
  });
}

// 一発ビルド。native のみビルド後に降格して**別パス**へ書き出す（未降格を配らない, ADR-0008 §3）。
// 返り値は `target` 向けに配信/入稿するバンドルの絶対パス。
export async function buildForTarget(config: TorimiConfig, target: Target, cwd: string): Promise<string> {
  await runShell(config.build, cwd);
  const bundleAbs = join(cwd, config.bundle);
  if (target === 'web') return bundleAbs;

  const loweredAbs = join(cwd, loweredBundlePath(config.bundle));
  const { classKeywordsLeft, size } = await lowerFileTo(bundleAbs, loweredAbs);
  console.log(`torimi: lowered for Hermes (class keywords left: ${classKeywordsLeft}, size ${size})`);
  return loweredAbs;
}
