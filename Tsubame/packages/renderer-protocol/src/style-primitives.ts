/**
 * 生成された `HayateStyle`/`StylePatch`（`./generated/style-types.ts`）から参照される
 * プリミティブ値型。`style_tags.json`/`enums.json` から機械的に導出できないため手書きで保持する。
 */
export type HayateDimension = number | `${number}px` | `${number}%` | `${number}fr` | 'auto';

/**
 * 単一の CSS box-shadow レイヤー（ADR-0095）。offset/blur/spread は CSS px、
 * `color` は任意の CSS カラー文字列、`inset` は内側シャドウを選択する。
 */
export interface HayateShadow {
  offsetX: number;
  offsetY: number;
  blur: number;
  spread: number;
  color: string;
  inset: boolean;
}

/**
 * Grid アイテムの1軸ぶんの配置端（CSS `grid-column` / `grid-row` の start/end、
 * issue #495）。`'auto'`/省略は自動配置、`number` は明示グリッド線（1 始まり、負値
 * は末尾から）、`{ span: n }` は `n` トラックぶんの占有。
 */
export type HayateGridLine = 'auto' | number | { readonly span: number };

/**
 * Grid アイテムの1軸ぶんの配置（CSS `grid-column` / `grid-row`）。`start` / `end`
 * の2スロットを持ち、省略した端は `auto`（自動配置）。
 */
export interface HayateGridPlacement {
  readonly start?: HayateGridLine;
  readonly end?: HayateGridLine;
}
