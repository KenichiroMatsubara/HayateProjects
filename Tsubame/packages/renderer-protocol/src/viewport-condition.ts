/**
 * プロパティ単位のスタイルバリアントに対するビューポート条件（ADR-0081）。
 *
 * 各軸は px で AND 結合する。`minWidth`/`minHeight` は閉区間で一致し
 * （`actual >= min*`）、`maxWidth`/`maxHeight` も閉区間で一致する
 * （`actual <= max*`）。CSS の `@media (min-width: ...)` /
 * `(max-width: ...)` 等に倣う。
 */
export interface ViewportCondition {
  readonly minWidth?: number;
  readonly maxWidth?: number;
  readonly minHeight?: number;
  readonly maxHeight?: number;
}
