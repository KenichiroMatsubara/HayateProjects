import type { ElementKind } from '@tsubame/renderer-protocol';

/**
 * Element 語彙（React Native 語彙）から HTML タグへのマッピング。
 * spec §5 の対応表に従う。HTML タグ名は Tsubame API には露出しない。
 */
const TAG_BY_KIND: Record<ElementKind, string> = {
  view: 'div',
  text: 'span',
  image: 'img',
  button: 'button',
  'text-input': 'input',
  'scroll-view': 'div',
};

/**
 * kind に対応する DOM 要素を生成する。`scroll-view` は overflow:auto を
 * 付与した div として実体化する（spec §5）。
 */
export function createDomElement(
  doc: Document,
  kind: ElementKind,
): HTMLElement {
  const el = doc.createElement(TAG_BY_KIND[kind]);
  if (kind === 'scroll-view') {
    el.style.overflow = 'auto';
  }
  return el;
}
