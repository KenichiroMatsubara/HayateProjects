import { describe, it, expect, vi } from 'vitest';
import type { ReactNode } from 'react';
import { HayateRenderer } from '@torimi/tsubame-renderer-hayate';
import { StubHayate, manualScheduler } from '@torimi/tsubame-renderer-hayate/test-helpers';
import { createTsubameRoot } from './mount.js';

/**
 * `TsubameInstance`（instance.ts）は、subtree の構造片付けを `IRenderer.removeChild` に
 * 委ね、各 instance 自身のリスナ解除は react-reconciler の `detachDeletedInstance`
 * （削除 subtree の各 host instance ごとに個別に呼ばれる）に委ねる、という設計を取る
 * （構造を辿らない）。この統合テストは、その設計が単一階層だけでなく多階層の subtree
 * 削除でも実際の `HayateRenderer` に対して成り立つことを確認する。
 */
describe('tsubame-react + HayateRenderer: multi-level subtree removal', () => {
  it('unsubscribes listeners at every depth of a removed subtree, not just its root', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    const grandparentClick = vi.fn();
    const parentClick = vi.fn();
    const childClick = vi.fn();

    function Tree({ show }: { show: boolean }): ReactNode {
      return (
        <view>
          {show && (
            <view onClick={grandparentClick}>
              <view onClick={parentClick}>
                <view onClick={childClick} />
              </view>
            </view>
          )}
        </view>
      );
    }

    const root = createTsubameRoot(renderer);
    root.render(<Tree show={true} />);

    expect(hayate.registeredListeners).toHaveLength(3);
    const listenerIds = hayate.registeredListeners.map((l) => l.listenerId);

    // 最上位の <view onClick={grandparentClick}> ごと外す。react-reconciler は
    // 削除された 3 インスタンスそれぞれに対して detachDeletedInstance を呼ぶはず。
    root.render(<Tree show={false} />);

    // 3 つの listenerId いずれについても、stale delivery を模してもハンドラは呼ばれない
    // = HayateRenderer 内部の listeners map からすべて削除されている。
    for (const listenerId of listenerIds) {
      hayate.events = [[listenerId, 0, 0, 0, 0]];
      sched.tick();
    }

    expect(grandparentClick).not.toHaveBeenCalled();
    expect(parentClick).not.toHaveBeenCalled();
    expect(childClick).not.toHaveBeenCalled();
  });
});
