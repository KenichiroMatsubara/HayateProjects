import type {
  DrawProperty,
  HayateCssStyle,
  EventHandler,
  UserSelect,
  ViewportCondition,
} from '@tsubame/renderer-protocol';

/**
 * ビューポート条件付きスタイル変種（ADR-0081）。DOM Renderer では本物の
 * `@media (min-width: …)` ルールへ、Canvas Renderer では viewport 評価へ落ちる。
 * Hayate CSS にはセレクタ・スタイルシートが無いため、`@media` は raw CSS では
 * なくこの型付き宣言として要素ごとに載せる（CONTEXT.md / 意味論パリティ）。
 */
export interface StyleVariant {
  condition: ViewportCondition;
  style: HayateCssStyle;
}

/**
 * Tsubame の Element 語彙に対する JSX 型定義。
 *
 * グローバル JSX 名前空間を宣言し、`<view>` / `<text>` 等の Element 語彙を
 * TSX で型付けする。デモ / アプリ側の tsconfig は `jsx: "preserve"` とし、
 * solid-js の jsxImportSource は設定しない（本宣言と衝突するため）。
 */
export interface TsubameProps {
  style?: HayateCssStyle;
  /** ビューポート条件付きスタイル（ADR-0081）。宣言順に適用される。 */
  styleVariants?: readonly StyleVariant[];
  onClick?: EventHandler;
  onInput?: EventHandler;
  onKeyDown?: EventHandler;
  onFocus?: EventHandler;
  onBlur?: EventHandler;
  children?: unknown;
}

declare global {
  // eslint-disable-next-line @typescript-eslint/no-namespace
  namespace JSX {
    type Element = unknown;
    interface ElementClass {
      // SolidJS は class component を持たないが、型解決のため空で宣言する。
      _?: never;
    }
    interface ElementChildrenAttribute {
      children: Record<string, never>;
    }
    interface IntrinsicElements {
      /** `draw`: 命令的 2D 描画の painter（#730 / ADR-0141）。view 限定（carriesDraw）。 */
      view: TsubameProps & { 'user-select'?: UserSelect; draw?: DrawProperty | null };
      text: TsubameProps & { 'user-select'?: UserSelect };
      image: TsubameProps & { src?: string; 'user-select'?: UserSelect };
      button: TsubameProps & { 'user-select'?: UserSelect };
      'text-input': TsubameProps & {
        value?: string;
        placeholder?: string;
        disabled?: boolean;
        multiline?: boolean;
      };
      'scroll-view': TsubameProps & { 'user-select'?: UserSelect };
    }
  }
}
