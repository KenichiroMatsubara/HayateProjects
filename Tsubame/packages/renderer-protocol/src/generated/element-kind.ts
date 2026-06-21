// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @hayate/protocol-spec（element_kinds.defaultCursor / defaultUserSelect）

import type { ElementKind, UserSelect } from '../element.js';

/** 要素種別ごとの UA デフォルトカーソル（CSS キーワード）（ADR-0105）。Canvas
 *  （Hayate コアの `resolve_cursor`）と共有する単一ソース。未指定 = デフォルトなし。 */
const DEFAULT_CURSOR: Partial<Record<ElementKind, string>> = {"button":"pointer","text-input":"text"};

/** `cursor` が明示指定されていない場合の `kind` の UA デフォルトカーソル。指定時は undefined。 */
export function elementKindDefaultCursor(kind: ElementKind): string | undefined {
  return DEFAULT_CURSOR[kind];
}

/** 要素種別ごとの UA デフォルト `user-select`（ADR-0108）。Canvas（Hayate コアの
 *  `default_user_select`）と共有する単一ソース。未指定 = `none`。 */
const DEFAULT_USER_SELECT: Partial<Record<ElementKind, UserSelect>> = {"view":"text","text":"text","image":"none","button":"none","text-input":"text","scroll-view":"text"};

/** 明示的な値が設定されていない場合の `kind` の UA デフォルト `user-select`（ADR-0108）。 */
export function elementKindDefaultUserSelect(kind: ElementKind): UserSelect {
  return DEFAULT_USER_SELECT[kind] ?? 'none';
}
