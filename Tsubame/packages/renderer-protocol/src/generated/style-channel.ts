// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @torimi/hayate-protocol-spec（style_tags.inherit / element_kinds.carriesTextLocal）

import type { StylePatch } from '../style.js';
import type { ElementKind } from '../element.js';

/** チャネル1の text-local スタイルキー（Style Channel; ADR-0065 / ADR-0002）。 */
const TEXT_LOCAL_KEYS: ReadonlySet<keyof StylePatch> = new Set(["fontSize","color","fontFamily","fontWeight","fontStyle","textDecoration"] as (keyof StylePatch)[]);

/** `key` がチャネル1の text-local スタイルかどうか。 */
export function isTextLocal(key: string): boolean {
  return TEXT_LOCAL_KEYS.has(key as keyof StylePatch);
}

/** チャネル1のスタイルを CSS として保持する要素種別（Text-Local Carrier）。 */
const TEXT_LOCAL_CARRIERS: ReadonlySet<ElementKind> = new Set(["text","text-input"] as ElementKind[]);

/** `kind` がチャネル1の text-local スタイルを CSS として保持するかどうか。 */
export function carriesTextLocal(kind: ElementKind): boolean {
  return TEXT_LOCAL_CARRIERS.has(kind);
}
