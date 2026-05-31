import type { IRenderer } from '@tsubame/renderer-protocol';
import { render } from './renderer.js';
import { setActiveRenderer } from './active-renderer.js';
import { createElementNode, type TsubameNode } from './node.js';

export interface RenderTsubameOptions {
  /**
   * リサイズを監視する DOM 要素。
   * 省略時は `window` の resize イベントを使う。
   * Canvas Renderer には canvas 要素を、DOM Renderer には container を渡す。
   */
  element?: Element;
}

/**
 * 指定 {@link IRenderer}（DOM / Canvas）にコンポーネントツリーをマウントする。
 *
 * `element` を渡すと ResizeObserver でサーフェスサイズを自動追従する。
 * アプリ側はウィンドウサイズを意識する必要がない。
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
  const root = createElementNode(rootId);

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
