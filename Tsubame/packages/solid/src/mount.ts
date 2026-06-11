import type { IRenderer } from '@tsubame/renderer-protocol';
import { render } from './renderer.js';
import { setActiveRenderer } from './active-renderer.js';
import { createElementNode, type TsubameNode } from './node.js';

export interface RenderTsubameOptions {
  /**
   * リサイズを監視する DOM 要素（DOM Renderer の container など）。
   * Canvas Renderer は ADR-0007 で自身が ResizeObserver を所有するため不要。
   * 省略時は `window` の resize イベントを使う。
   */
  element?: Element;
}

/**
 * 指定 {@link IRenderer}（DOM / Canvas）にコンポーネントツリーをマウントする。
 *
 * DOM Renderer では `element` を渡すと ResizeObserver でサーフェスサイズを自動追従する。
 * Canvas Renderer はレンダラー側がビューポート追従を担当する（ADR-0007）。
 *
 * @returns dispose 関数。SolidJS の reactive スコープとリサイズ監視を破棄する。
 */
export function renderTsubame(
  code: () => unknown,
  renderer: IRenderer,
  options?: RenderTsubameOptions,
): () => void {
  setActiveRenderer(renderer);
  const rootId = renderer.createElement('view');
  renderer.setRoot(rootId);
  const root = createElementNode(rootId, 'view');

  // RAF でデバウンスしてリサイズを renderer に通知する
  let rafHandle: number | null = null;
  const notifyResize = (w: number, h: number): void => {
    if (rafHandle !== null) cancelAnimationFrame(rafHandle);
    rafHandle = requestAnimationFrame(() => {
      rafHandle = null;
      renderer.resize(w, h);
    });
  };

  let cleanupResize: (() => void) | null = null;
  const el = options?.element;

  if (el !== undefined && typeof ResizeObserver !== 'undefined') {
    const ro = new ResizeObserver((entries) => {
      const entry = entries[0];
      if (!entry) return;
      const { width, height } = entry.contentRect;
      notifyResize(Math.round(width), Math.round(height));
    });
    ro.observe(el);
    cleanupResize = () => ro.disconnect();
  } else {
    const handler = (): void => notifyResize(window.innerWidth, window.innerHeight);
    window.addEventListener('resize', handler);
    cleanupResize = () => window.removeEventListener('resize', handler);
  }

  const dispose = render(code as () => TsubameNode, root);
  return () => {
    if (rafHandle !== null) cancelAnimationFrame(rafHandle);
    cleanupResize?.();
    dispose();
  };
}
