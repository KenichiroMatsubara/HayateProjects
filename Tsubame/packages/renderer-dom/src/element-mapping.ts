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

// UA stylesheet の素地が Tsubame の StylePatch 仕様を侵食しないよう、
// 生成時に最小リセットを当てる（特に <button> / <input> の native chrome 対策）。
const UA_RESET =
  'margin:0;padding:0;border:0;background:none;font:inherit;color:inherit;' +
  'box-sizing:border-box;appearance:none;-webkit-appearance:none;outline:none;' +
  'text-align:inherit;cursor:inherit;';

/**
 * kind に対応する DOM 要素を生成する。`scroll-view` は overflow:auto を
 * 付与した div として実体化する（spec §5）。
 *
 * button / text-input には水平パディング・テキスト中央寄せ等の最小デフォルト
 * を入れる（Canvas Renderer の mock-hayate.ts が button kind に padding 16/10
 * をハードコードしているため、両 Renderer の見た目を揃える）。
 */
export function createDomElement(
  doc: Document,
  kind: ElementKind,
): HTMLElement {
  const el = doc.createElement(TAG_BY_KIND[kind]);
  el.style.cssText = UA_RESET;
  if (kind === 'button') {
    el.style.cursor = 'pointer';
    el.style.display = 'inline-flex';
    el.style.alignItems = 'center';
    el.style.justifyContent = 'center';
    el.style.padding = '0 14px';
    el.style.whiteSpace = 'nowrap';
  }
  if (kind === 'text-input') {
    el.style.padding = '0 12px';
  }
  if (kind === 'scroll-view') el.style.overflow = 'auto';
  return el;
}
