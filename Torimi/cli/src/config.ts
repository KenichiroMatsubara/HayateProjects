import { access } from 'node:fs/promises';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

import { DEFAULT_WATCH_DIR } from './constants.js';

// torimi.config.* はフラット（ADR-0008 §3）: `build`（一発ビルドコマンド, 不透明）と
// `bundle`（その出力パス）だけ。`watch`（監視ソースディレクトリ）は optional。per-target
// 分岐は持たない — native/web の差（降格・ポート）は CLI 側の知識。
export interface TorimiConfig {
  /** 一発ビルドコマンド（例 `vite build --config vite.config.torimi.ts`）。CLI は不透明に実行する。 */
  readonly build: string;
  /** build が書き出す単一 App Bundle の、cwd 相対パス。 */
  readonly bundle: string;
  /** `torimi dev` が監視するソースディレクトリ（cwd 相対、既定 `src`）。 */
  readonly watch: string;
}

const CONFIG_BASENAMES = ['torimi.config.mjs', 'torimi.config.js'];

export async function findConfigPath(cwd: string): Promise<string> {
  for (const name of CONFIG_BASENAMES) {
    const candidate = join(cwd, name);
    try {
      await access(candidate);
      return candidate;
    } catch {
      // 次の候補へ
    }
  }
  throw new Error(`torimi: no ${CONFIG_BASENAMES.join(' or ')} found in ${cwd}`);
}

export function normalizeConfig(raw: unknown): TorimiConfig {
  if (!raw || typeof raw !== 'object') throw new Error('torimi.config: default export must be an object');
  const { build, bundle, watch } = raw as Record<string, unknown>;
  if (typeof build !== 'string' || build.trim() === '') {
    throw new Error('torimi.config: `build` must be a non-empty string');
  }
  if (typeof bundle !== 'string' || bundle.trim() === '') {
    throw new Error('torimi.config: `bundle` must be a non-empty string');
  }
  if (watch !== undefined && typeof watch !== 'string') {
    throw new Error('torimi.config: `watch` must be a string');
  }
  return { build, bundle, watch: watch ?? DEFAULT_WATCH_DIR };
}

export async function loadTorimiConfig(cwd: string): Promise<TorimiConfig> {
  const path = await findConfigPath(cwd);
  const mod = (await import(pathToFileURL(path).href)) as { default?: unknown };
  return normalizeConfig(mod.default ?? mod);
}
