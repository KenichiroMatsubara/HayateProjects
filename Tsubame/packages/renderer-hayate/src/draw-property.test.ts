import { describe, it, expect, vi } from 'vitest';
import { DRAW_OP, EVENT_KIND, OP } from '@torimi/tsubame-protocol-generated/protocol';
import { Paint, Path } from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas, DrawSize } from '@torimi/tsubame-renderer-protocol';
import { HayateRenderer } from './hayate-renderer.js';
import { StubHayate, manualScheduler } from './test-helpers/stub-hayate.js';

// #730 / ADR-0141・0143: `draw` property（painter）の配線。painter を書けば
// Hayate Renderer（GPU Canvas 経路）で絵が出る縦貫通。renderer は per-element
// layout size イベント（#725）を内部購読し、受信時に painter を実サイズで呼んで
// recorder（#729）の display list を `draws` チャネルに載せ、次フレームの
// mutation で送る（初回・リサイズ時の 1 フレーム遅延は仕様 = ResizeObserver パリティ）。

/** ボックスいっぱいの矩形を既定 Paint（黒 fill）で塗る最小 painter。 */
function fillBox(canvas: DrawCanvas, size: DrawSize): void {
  canvas.drawPath(new Path().addRect(0, 0, size.width, size.height), new Paint());
}

/** 既定 Paint の FILL パケット（COLOR タグ + 不透明黒）。 */
const FILL_DEFAULT_PAINT = [DRAW_OP.FILL, 5, 0, 0, 0, 0, 1];

function setup() {
  const hayate = new StubHayate();
  const sched = manualScheduler();
  const renderer = new HayateRenderer({ raw: hayate, ...sched });
  renderer.start();
  return { hayate, sched, renderer };
}

describe('draw property → painter → draws channel (#730)', () => {
  it('calls paint(canvas, size) with the laid-out size on layout_resize and ships the display list next frame', () => {
    const { hayate, sched, renderer } = setup();
    const view = renderer.createElement('view');
    renderer.setRoot(view);

    const sizes: DrawSize[] = [];
    renderer.setDraw(view, (canvas, size) => {
      sizes.push(size);
      fillBox(canvas, size);
    });

    // draw の設定は per-element layout size イベント（#725）の内部リスナを登録する。
    expect(hayate.registeredListeners).toEqual([
      { elementId: view as number, eventKind: EVENT_KIND.LAYOUT_RESIZE, listenerId: 1 },
    ]);
    // レイアウト未確定のうちは painter は呼ばれない（サイズを知る口が無い・ADR-0143）。
    expect(sizes).toEqual([]);

    // frame 1: 構造 mutation が flush され、レイアウト確定で layout_resize が届く。
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 120, 80]];
    sched.tick();
    expect(sizes).toEqual([{ width: 120, height: 80 }]);

    // frame 2（次フレームの mutation・1 フレーム遅延）: SET_DRAW + display list が載る。
    sched.tick();
    const batch = hayate.mutations.at(-1)!;
    expect(batch.ops).toContain(OP.SET_DRAW);
    expect(batch.draws).toEqual([DRAW_OP.RECT, 0, 0, 120, 80, ...FILL_DEFAULT_PAINT]);
  });

  it('skips re-record and wire send when shouldRepaint(old) is false (reactive invalidation)', () => {
    const { hayate, sched, renderer } = setup();
    const view = renderer.createElement('view');
    renderer.setRoot(view);

    const makePainter = (repaint: boolean) => ({
      paint: vi.fn(fillBox),
      shouldRepaint: vi.fn(() => repaint),
    });

    const first = makePainter(true);
    renderer.setDraw(view, first);
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 50, 40]];
    sched.tick(); // 初回 paint
    sched.tick(); // SET_DRAW 送信
    const sent = hayate.mutations.length;

    // shouldRepaint が false: 再記録も wire 送信も起きない。
    const declined = makePainter(false);
    renderer.setDraw(view, declined);
    expect(declined.shouldRepaint).toHaveBeenCalledWith(first);
    expect(declined.paint).not.toHaveBeenCalled();
    sched.tick();
    expect(hayate.mutations.length).toBe(sent);

    // shouldRepaint が true: 追加の resize なしで確定済みサイズで再記録して送る。
    const accepted = makePainter(true);
    renderer.setDraw(view, accepted);
    expect(accepted.paint).toHaveBeenCalledWith(expect.anything(), { width: 50, height: 40 });
    sched.tick();
    const batch = hayate.mutations.at(-1)!;
    expect(batch.ops).toContain(OP.SET_DRAW);
  });

  it('treats the function sugar by identity: same function does not resend, a new one does', () => {
    const { hayate, sched, renderer } = setup();
    const view = renderer.createElement('view');
    renderer.setRoot(view);

    const painter = vi.fn(fillBox);
    renderer.setDraw(view, painter);
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 50, 40]];
    sched.tick();
    sched.tick();
    expect(painter).toHaveBeenCalledTimes(1);
    const sent = hayate.mutations.length;

    // 同一関数の再設定（solid/react の再実行で普通に起きる）: 再記録・再送信なし。
    renderer.setDraw(view, painter);
    sched.tick();
    expect(painter).toHaveBeenCalledTimes(1);
    expect(hayate.mutations.length).toBe(sent);

    // 別関数: identity が変わったので再記録して送る。
    const next = vi.fn(fillBox);
    renderer.setDraw(view, next);
    expect(next).toHaveBeenCalledTimes(1);
    sched.tick();
    expect(hayate.mutations.at(-1)!.ops).toContain(OP.SET_DRAW);
  });

  it('does not resend draws for a layout_resize with an unchanged size (no wasted send)', () => {
    const { hayate, sched, renderer } = setup();
    // 継続 pending のあるアプリ（transition 等）を模し、フレームループを回し続ける。
    // 無駄送信の有無は「フレームが回ること」ではなく mutation の増分で観測する。
    hayate.pendingVisualWork = true;
    const view = renderer.createElement('view');
    renderer.setRoot(view);

    const painter = vi.fn(fillBox);
    renderer.setDraw(view, painter);
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 50, 40]];
    sched.tick();
    sched.tick();
    expect(painter).toHaveBeenCalledTimes(1);
    const sent = hayate.mutations.length;

    // core は size 非変化 commit では発火しない（#725）が、renderer 側でも同サイズ
    // 通知を無駄送信に増幅しない（防御的パリティ）。
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 50, 40]];
    sched.tick();
    sched.tick();
    expect(painter).toHaveBeenCalledTimes(1);
    expect(hayate.mutations.length).toBe(sent);

    // サイズが変わる resize では新サイズで再 paint して送る。
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 70, 40]];
    sched.tick();
    expect(painter).toHaveBeenCalledTimes(2);
    expect(painter).toHaveBeenLastCalledWith(expect.anything(), { width: 70, height: 40 });
    sched.tick();
    expect(hayate.mutations.at(-1)!.ops).toContain(OP.SET_DRAW);
  });

  it('clears the drawing when the draw property is set to null', () => {
    const { hayate, sched, renderer } = setup();
    const view = renderer.createElement('view');
    renderer.setRoot(view);

    renderer.setDraw(view, fillBox);
    hayate.events = [[1, EVENT_KIND.LAYOUT_RESIZE, view as number, 50, 40]];
    sched.tick();
    sched.tick();

    // null で空 display list を送って消す。以後の resize では paint しない。
    renderer.setDraw(view, null);
    sched.tick();
    const batch = hayate.mutations.at(-1)!;
    expect(batch.ops).toContain(OP.SET_DRAW);
    expect(batch.draws).toEqual([]);
  });
});
