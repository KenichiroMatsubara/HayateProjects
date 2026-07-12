import type { IRenderer } from '@torimi/tsubame-renderer-protocol';
import { withTextLocalGate } from '@torimi/tsubame-renderer-protocol';
import { render } from './renderer.js';
import { setActiveRenderer } from './active-renderer.js';
import { createElementNode, type TsubameNode } from './node.js';

/**
 * 指定 {@link IRenderer}（DOM / Canvas）にコンポーネントツリーをマウントする。
 *
 * viewport 追従（resize）は Tsubame の責務ではない。DOM はブラウザの CSS リフロー
 * と `@media` で、Canvas は Web なら hayate-adapter-web、Android なら native ループが
 * `tree.set_viewport` を直接駆動する（ADR-0080, native 延長は issue #475）。よって
 * `renderTsubame` は resize を一切配線しない。
 *
 * @returns dispose 関数。SolidJS の reactive スコープを破棄する。
 */
export function renderTsubame(code: () => unknown, target: IRenderer): () => void {
  // Style Channel ゲートを選択したレンダラーの手前で一度だけ適用する
  // （ADR-0008）。各レンダラーはフィルタ済みのパッチを受け取るので、
  // Semantics Parity は構造的に成立し、新レンダラーは独自ゲート不要。
  const renderer = withTextLocalGate(target);
  setActiveRenderer(renderer);
  const rootId = renderer.createElement('view');
  renderer.setRoot(rootId);
  const root = createElementNode(rootId, 'view');

  const dispose = render(code as () => TsubameNode, root);
  return () => {
    dispose();
  };
}
