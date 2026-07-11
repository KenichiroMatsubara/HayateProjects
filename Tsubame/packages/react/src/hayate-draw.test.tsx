import { describe, it, expect } from 'vitest';
import { DRAW_OP, EVENT_KIND, OP } from '@torimi/tsubame-protocol-generated/protocol';
import { Paint, Path } from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas, DrawSize } from '@torimi/tsubame-renderer-protocol';
import { HayateRenderer } from '@torimi/tsubame-renderer-hayate';
import { StubHayate, manualScheduler } from '@torimi/tsubame-renderer-hayate/test-helpers';
import { createTsubameRoot } from './mount.js';

// #730 AC: tsubame-react から draw 付き view がマウントでき、Hayate Renderer 経由で
// 描画される（painter → layout size イベント → recorder → draws チャネル）。

describe('tsubame-react + HayateRenderer: draw property (#730)', () => {
  it('mounts a view with a draw painter and ships its display list through the draws channel', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    const sizes: DrawSize[] = [];
    const painter = (canvas: DrawCanvas, size: DrawSize): void => {
      sizes.push(size);
      canvas.drawPath(new Path().addCircle(size.width / 2, size.height / 2, 10), new Paint());
    };

    const root = createTsubameRoot(renderer);
    root.render(<view draw={painter} />);

    // draw 付き view は per-element layout size イベント（#725）を内部購読している。
    const drawListener = hayate.registeredListeners.find(
      (l) => l.eventKind === EVENT_KIND.LAYOUT_RESIZE,
    );
    expect(drawListener).toBeDefined();

    // レイアウト確定 → painter が実サイズで呼ばれ、次フレームで SET_DRAW が届く。
    hayate.events = [
      [drawListener!.listenerId, EVENT_KIND.LAYOUT_RESIZE, drawListener!.elementId, 200, 100],
    ];
    sched.tick();
    expect(sizes).toEqual([{ width: 200, height: 100 }]);

    sched.tick();
    const batch = hayate.mutations.at(-1)!;
    expect(batch.ops).toContain(OP.SET_DRAW);
    expect(batch.draws.slice(0, 4)).toEqual([DRAW_OP.CIRCLE, 100, 50, 10]);
  });
});
