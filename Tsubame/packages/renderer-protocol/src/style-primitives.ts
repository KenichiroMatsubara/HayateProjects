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
