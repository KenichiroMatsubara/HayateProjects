import type { IRenderer } from '@tsubame/renderer-protocol';
import { render } from './renderer.js';
import { setActiveRenderer } from './active-renderer.js';
import { createElementNode, type TsubameNode } from './node.js';

/**
 * 指定 {@link IRenderer}（DOM / Canvas）にコンポーネントツリーをマウントする。
 *
 * Adapter コードはどちらの Renderer を使うかを意識しない。同一の `code` を
 * 別 Renderer で `renderTsubame` し直せば、レンダリング先だけを差し替えられる
 * （Tsubame の訴求点）。
 *
 * @returns dispose 関数。SolidJS の reactive スコープを破棄する。
 */
export function renderTsubame(
  code: () => unknown,
  renderer: IRenderer,
): () => void {
  setActiveRenderer(renderer);
  const rootId = renderer.createElement('view');
  renderer.setRoot(rootId);
  const root = createElementNode(rootId);
  return render(code as () => TsubameNode, root);
}
