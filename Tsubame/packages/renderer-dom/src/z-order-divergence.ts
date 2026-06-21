import type { ElementId, HayateStyle } from '@tsubame/renderer-protocol';

/**
 * ブラウザの暗黙的なスタッキングコンテキストを生み、Canvas/Hayate の Z-Order セマンティクスと
 * 乖離しうるスタイル。新しい Element スタイルプロパティ（例: `transform`）の追加時に拡張する。
 */
export const Z_ORDER_DIVERGENCE_PROPERTIES: ReadonlySet<keyof HayateStyle> = new Set([
  'opacity',
  // 'transform' — Element スタイルが transform を持ったら追加する
]);

const warned = new Set<string>();

function warningKey(elementId: ElementId, property: string): string {
  return `${elementId as number}:${property}`;
}

/**
 * dev で乖離するスタイルプロパティが適用されたとき、セッションにつき一度だけ警告する。
 * production では no-op。例外は投げない。
 */
export function warnZOrderDivergence(elementId: ElementId, property: string): void {
  if (process.env.NODE_ENV === 'production') return;
  if (!Z_ORDER_DIVERGENCE_PROPERTIES.has(property as keyof HayateStyle)) return;

  const key = warningKey(elementId, property);
  if (warned.has(key)) return;
  warned.add(key);

  console.warn(
    `[Tsubame DOM Renderer] Setting "${property}" on element ${elementId as number} may cause Z-Order divergence from Canvas/Hayate semantics. See Tsubame/docs/adr/0006-dom-z-order-rn-web-emulation.md`,
  );
}

/** @internal 重複排除状態をリセットする。テスト専用 */
export function resetZOrderDivergenceWarnings(): void {
  warned.clear();
}
