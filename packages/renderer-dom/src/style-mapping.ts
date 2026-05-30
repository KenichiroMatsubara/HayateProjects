import type { HayateStyle, StylePatch } from '@tsubame/renderer-protocol';

/**
 * HayateStyle の各プロパティを CSS プロパティ名（camelCase）へ対応付ける。
 * MVP では DOM Renderer 上ではプロパティ名がほぼそのまま CSS に一致する。
 */
const CSS_PROP: Record<keyof HayateStyle, string> = {
  width: 'width',
  height: 'height',
  display: 'display',
  flexDirection: 'flexDirection',
  alignItems: 'alignItems',
  justifyContent: 'justifyContent',
  gap: 'gap',
  color: 'color',
  backgroundColor: 'backgroundColor',
  borderRadius: 'borderRadius',
  opacity: 'opacity',
  fontSize: 'fontSize',
  fontWeight: 'fontWeight',
};

/** px 単位を付与する長さ系プロパティ。 */
const PX_PROPS = new Set<keyof HayateStyle>([
  'width',
  'height',
  'gap',
  'borderRadius',
  'fontSize',
]);

function format(key: keyof HayateStyle, value: NonNullable<unknown>): string {
  if (PX_PROPS.has(key) && typeof value === 'number') {
    return `${value}px`;
  }
  return String(value);
}

/**
 * {@link StylePatch} を DOM 要素のインラインスタイルへ適用する。
 *
 * - 値を持つプロパティは上書き
 * - `null` は空文字を設定してインラインスタイルを解除（デフォルトへ戻す）
 *
 * `display: 'flex'` の DOM では未指定 prop の挙動が CSS のカスケードに従う点に
 * 注意（Protocol の StylePatch セマンティクスは「未指定＝変更なし」）。
 */
export function applyStylePatch(el: HTMLElement, patch: StylePatch): void {
  const style = el.style as unknown as Record<string, string>;
  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;
    const cssProp = CSS_PROP[k];
    style[cssProp] = value === null ? '' : format(k, value);
  }
}
