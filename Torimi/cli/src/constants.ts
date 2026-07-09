import { extname } from 'node:path';

// ── Targets ──────────────────────────────────────────────────────────────────
// 1 起動 = 1 ターゲット（ADR-0008 §2）。native 既定（既存のネイティブ焼き込み既定に整合）。
export const TARGETS = ['native', 'web'] as const;
export type Target = (typeof TARGETS)[number];
export const DEFAULT_TARGET: Target = 'native';

// ── Ports（名前付き定数, ADR-0008 §2 / マジックナンバーなし）──────────────────
// native 既定はネイティブに焼き込まれた DEFAULT_DEV_SERVER_PORT（dev_server_target.rs）に
// 合わせる — 端末 UI で URL 未入力のエミュレータが 10.0.2.2:5179 へ落ちる気配りを壊さない。
export const NATIVE_DEV_PORT = 5179;
// web ホスト経路の既定ポート（playwright の TORIMI_DEV_PORT 既定と一致）。
export const WEB_DEV_PORT = 5181;

// `torimi dev` が監視する既定ソースディレクトリ（torimi.config.watch で上書き可）。
export const DEFAULT_WATCH_DIR = 'src';

// 連続したファイル書き込み（保存・エディタ一時ファイル）を 1 回の再ビルドに畳む猶予。
export const REBUILD_DEBOUNCE_MS = 120;

export function portForTarget(target: Target): number {
  return target === 'native' ? NATIVE_DEV_PORT : WEB_DEV_PORT;
}

export function resolveTarget(arg: string | undefined): Target {
  if (arg === undefined) return DEFAULT_TARGET;
  if ((TARGETS as readonly string[]).includes(arg)) return arg as Target;
  throw new Error(`torimi: unknown target "${arg}" (expected one of: ${TARGETS.join(', ')})`);
}

// native は Hermes 降格済みバンドルを **別パス** に置いて配信する（未降格を配らない, ADR-0008 §3）。
// ビルド出力の拡張子直前に `.hermes` を差し込んで導出する（config は 1 パスのまま = per-target
// 分岐なし）。
export function loweredBundlePath(bundle: string): string {
  const ext = extname(bundle);
  return ext ? `${bundle.slice(0, -ext.length)}.hermes${ext}` : `${bundle}.hermes`;
}
