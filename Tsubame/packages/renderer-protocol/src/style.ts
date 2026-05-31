/**
 * Hayate CSS のレイアウトプロパティ列挙。レイアウト系は Taffy の
 * CSS Flexbox 実装を仕様とする。MVP では Flexbox サブセットのみ対応。
 */
export type Display = 'flex' | 'none';
export type FlexDirection = 'row' | 'column';
export type AlignItems = 'flex-start' | 'flex-end' | 'center' | 'stretch';
export type JustifyContent =
  | 'flex-start'
  | 'flex-end'
  | 'center'
  | 'space-between'
  | 'space-around'
  | 'space-evenly';

/** CSS の font-weight に対応する数値ウェイト。 */
export type FontWeight = 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;

/**
 * Tsubame が Renderer 経由で扱うスタイル仕様（MVP サブセット）。
 *
 * Canvas Renderer 経由では Hayate の `style_packet.rs` TAG エンコーディングへ、
 * DOM Renderer では対応する CSS プロパティへ直接マッピングされる。
 * 長さ系プロパティ（width / height / gap / borderRadius / fontSize）の単位は
 * px とする。
 *
 * Grid・overflow・border・shadow 等は MVP 後に追加する。
 */
export interface HayateStyle {
  // --- レイアウト ---
  /** px 数値または `'100%'`（親コンテナに対する割合）。 */
  width: number | string;
  /** px 数値または `'100%'`（親コンテナに対する割合）。 */
  height: number | string;
  display: Display;
  flexDirection: FlexDirection;
  alignItems: AlignItems;
  justifyContent: JustifyContent;
  gap: number;
  /** Flexbox の flex-grow。残余空間の配分比率。デフォルト 0。 */
  flexGrow: number;

  // --- ビジュアル ---
  /** CSS color 文字列（例: `#1e90ff` / `rgb(30,144,255)`）。 */
  color: string;
  /** CSS color 文字列。 */
  backgroundColor: string;
  borderRadius: number;
  /** 0.0〜1.0。 */
  opacity: number;

  // --- テキスト ---
  fontSize: number;
  fontWeight: FontWeight;
}

/**
 * `IRenderer.setStyle` の第二引数。
 *
 * - 指定されたプロパティのみ上書き
 * - 未指定のプロパティは変更なし
 * - `null` はリセット（デフォルト値に戻す）
 *
 * 毎フレーム全プロパティを送るフル置換は行わない。
 */
export type StylePatch = {
  [K in keyof HayateStyle]?: HayateStyle[K] | null;
};
