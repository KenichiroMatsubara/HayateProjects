import type { ReactNode } from 'react';
import { ConcurrentRoot } from 'react-reconciler/constants';
import type { IRenderer } from '@tsubame/renderer-protocol';
import { withTextLocalGate } from '@tsubame/renderer-protocol';
import { createReconciler, type TsubameContainer } from './host-config.js';

/** {@link createTsubameRoot} が返すルートハンドル。 */
export interface TsubameRoot {
  /** コンポーネントツリーを描画する（同期 flush）。 */
  render(element: ReactNode): void;
  /** ツリーを破棄する（root を空にして flush）。 */
  unmount(): void;
}

const logError = (error: unknown): void => {
  // error boundary が無い場合のフォールバック。テストは IRenderer 境界を読むため、
  // ここでの握りつぶしはせず可視化だけ行う。
  // eslint-disable-next-line no-console
  console.error(error);
};

/**
 * 指定 {@link IRenderer} に束縛した reconciler root を生成する。
 *
 * `withTextLocalGate`（ADR-0008）を一度だけ適用し、root `view` を作って `setRoot` する。
 * renderer は root 生成時に束縛され、active-renderer グローバルは持たない（ADR-0010）。
 * `render` / `unmount` は `flushSync` で同期コミットするため、テストは flush 後すぐに
 * `IRenderer` 境界の記録を assert できる。
 */
export function createTsubameRoot(target: IRenderer): TsubameRoot {
  // Style Channel ゲートを選択したレンダラーの手前で一度だけ適用する（ADR-0008）。
  const renderer = withTextLocalGate(target);
  const reconciler = createReconciler(renderer);

  const rootId = renderer.createElement('view');
  renderer.setRoot(rootId);

  const container: TsubameContainer = { renderer, rootId };
  const root = reconciler.createContainer(
    container,
    ConcurrentRoot,
    null,
    false,
    null,
    '',
    logError,
    logError,
    logError,
    noop,
    null,
  );

  // react-reconciler 0.32 のランタイムは旧 `flushSync(fn)` を `flushSyncFromReconciler`
  // として公開する（@types は古い名前のまま）。同期コミットして、flush 後すぐに
  // `IRenderer` 境界を読めるようにする。
  const flushSync = (reconciler as unknown as {
    flushSyncFromReconciler: <R>(fn: () => R) => R;
  }).flushSyncFromReconciler;

  return {
    render(element: ReactNode): void {
      flushSync(() => {
        reconciler.updateContainer(element, root, null, null);
      });
    },
    unmount(): void {
      flushSync(() => {
        reconciler.updateContainer(null, root, null, null);
      });
    },
  };
}

function noop(): void {}

/**
 * React コンポーネントツリーを {@link IRenderer}（DOM / Canvas）越しに描画する。
 *
 * viewport 追従（resize）は Tsubame の責務ではない（DOM はブラウザの CSS リフローと
 * `@media`、Canvas は native ループが駆動する。ADR-0080 / ADR-0007）。`tsubame-solid` と
 * 対称に resize は配線しない。
 *
 * @returns dispose 関数。container（reconciler root）を破棄する。
 */
export function renderTsubame(element: ReactNode, target: IRenderer): () => void {
  const root = createTsubameRoot(target);
  root.render(element);
  return () => root.unmount();
}
