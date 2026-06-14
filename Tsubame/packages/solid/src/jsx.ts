import type { HayateCssStyle, EventHandler } from '@tsubame/renderer-protocol';

/**
 * Tsubame の Element 語彙に対する JSX 型定義。
 *
 * グローバル JSX 名前空間を宣言し、`<view>` / `<text>` 等の Element 語彙を
 * TSX で型付けする。デモ / アプリ側の tsconfig は `jsx: "preserve"` とし、
 * solid-js の jsxImportSource は設定しない（本宣言と衝突するため）。
 */
export interface TsubameProps {
  style?: HayateCssStyle;
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
      view: TsubameProps & { selectable?: boolean };
      text: TsubameProps & { selectable?: boolean };
      image: TsubameProps & { src?: string };
      button: TsubameProps;
      'text-input': TsubameProps & {
        value?: string;
        placeholder?: string;
        disabled?: boolean;
      };
      'scroll-view': TsubameProps & { selectable?: boolean };
    }
  }
}
