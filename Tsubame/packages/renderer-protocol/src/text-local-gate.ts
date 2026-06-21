import type { ElementKind } from './element.js';
import type { StylePatch } from './style.js';
import { isTextLocal, carriesTextLocal } from './generated/style-channel.js';

/**
 * Style Channel ゲート（ADR-0065 / ADR-0002）。チャンネル1の text-local プロップは
 * Text-Local Carrier の kind にのみ適用され、それ以外のプロップは常に適用される。
 * DOM レンダラ（CSS 書き込み前）と Canvas レンダラ（ワイヤ符号化前）が共通で参照する
 * 唯一のルールで、両者が暗黙に乖離しないことを保証する。
 */
export function shouldApplyTextLocalPatch(kind: ElementKind, patchKey: string): boolean {
  if (!isTextLocal(patchKey)) return true;
  return carriesTextLocal(kind);
}

/**
 * `kind` が運ばない text-local プロップを宣言順を保ったまま除去する。
 * Carrier（およびゲート対象キーを含まないパッチ）はそのまま通過する。
 */
export function gateTextLocalPatch(kind: ElementKind, patch: StylePatch): StylePatch {
  if (carriesTextLocal(kind)) return patch;

  const gated: Record<string, unknown> = {};
  for (const key in patch) {
    if (!shouldApplyTextLocalPatch(kind, key)) continue;
    gated[key] = patch[key as keyof StylePatch];
  }
  return gated as StylePatch;
}
