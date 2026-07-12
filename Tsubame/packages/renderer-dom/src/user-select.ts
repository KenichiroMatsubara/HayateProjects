import type { ElementKind, UserSelect } from '@torimi/tsubame-renderer-protocol';
import { elementKindDefaultUserSelect } from '@torimi/tsubame-renderer-protocol';

/**
 * 要素の CSS `user-select` 値を解決する（ADR-0108）。DOM Mode はブラウザの
 * ネイティブ選択を使い、`user-select` で範囲を制限する。
 *
 * この意味論は DOM Renderer と Hayate HTML Mode（Rust の `resolve_user_select`）の
 * 唯一の整合基準。共有コーパス `proto/spec/fixtures/user_select_parity.json` が
 * 両者のずれを防ぐ。
 *
 * 解決順序: 明示的 `user-select` → 要素種別の UA 既定 → (none/選択不可)。
 * 選択可能性は CSS と同じくオプトアウト:
 *
 * - `text-input` は常に自身の編集選択を持つので、明示値や種別既定に関わらず `text`。
 * - それ以外は、明示的 `user-select` があればそれ、無ければ種別既定
 *   （`view` / `text` / `scroll-view` = `text`、`image` / `button` = `none`）。
 * - `text` は CSS `text`、`none` は CSS `none`（サブツリーを除外）に対応。
 * - `contains` は選択可能だが包含境界を確立するため CSS `contain` に対応する
 *   （ADR-0108）。`user-select: contain` 対応ブラウザはネイティブ選択を要素内に
 *   閉じ込め、非対応ブラウザは値を無視するがコア側の境界クランプが同じ意味論を
 *   与える（意味論のみの整合）。
 */
export function resolveUserSelect(
  kind: ElementKind,
  explicit: UserSelect | undefined,
): 'text' | 'none' | 'contain' {
  if (kind === 'text-input') return 'text';
  const effective = explicit ?? elementKindDefaultUserSelect(kind);
  if (effective === 'none') return 'none';
  if (effective === 'contains') return 'contain';
  return 'text';
}
