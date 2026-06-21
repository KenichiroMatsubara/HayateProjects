import type { HayateStyle, StylePatch } from '@tsubame/renderer-protocol';

type DomStylePatchBase = Omit<StylePatch, 'opacity'>;

/**
 * DOM Renderer 用のスタイルパッチ。Canvas/Hayate の Z-Order セマンティクスと
 * 食い違いうるプロパティに IDE 警告を付ける。
 */
export type DomStylePatch = DomStylePatchBase & {
  /**
   * @deprecated ブラウザ CSS は opacity に対し暗黙の重ね合わせコンテキストを
   * 生成するため、DOM Renderer は Canvas/Hayate の Z-Order セマンティクスと
   * 食い違いうる。Tsubame/docs/adr/0006-dom-z-order-rn-web-emulation.md を参照。
   */
  opacity?: HayateStyle['opacity'] | null;
};
