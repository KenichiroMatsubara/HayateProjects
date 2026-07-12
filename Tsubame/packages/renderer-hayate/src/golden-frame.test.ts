import { afterEach, describe, expect, it } from 'vitest';
import { createElement, renderTsubame, setProp } from '@torimi/tsubame-solid';
import { HayateRenderer } from './hayate-renderer.js';
import { createNullHayate, type WasmHayateFixture } from './test-helpers/wasm-hayate.js';
import { manualScheduler } from './test-helpers/manual-scheduler.js';
import { captureGoldenFrame, type GoldenFrameSource } from './golden-frame.js';

describe('golden frame cross-seam harness (ADR-0079)', () => {
  let fixture: WasmHayateFixture | null = null;
  let dispose: (() => void) | null = null;

  afterEach(() => {
    dispose?.();
    dispose = null;
    fixture?.dispose();
    fixture = null;
  });

  it('mounts a focused, typed text-input and captures a tree/style/layout/a11y golden frame', async () => {
    fixture = await createNullHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: fixture.raw, ...sched });
    renderer.start();

    let inputId = 0;
    dispose = renderTsubame(() => {
      const input = createElement('text-input');
      setProp(input, 'style', {
        width: '120px',
        height: '32px',
        backgroundColor: '#ffffff',
      });
      setProp(input, 'value', 'Hi');
      inputId = input.id;
      return input;
    }, renderer);

    // Shadow Tree 再構成 -> Mutation Packet -> 実 WASM ElementTree。
    sched.tick(16);

    // 実際の pointer-down ヒットテストで text-input にフォーカスし、入力する。
    const [x, y, w, h] = Array.from(fixture.raw.element_get_bounds(inputId));
    fixture.raw.on_pointer_down(x! + w! / 2, y! + h! / 2);
    fixture.raw.on_text_input(inputId, '!');

    // 再レンダリング -> ElementTree。
    sched.tick(32);

    // IME（EditContext 着脱・候補窓 rect）はアダプタが `render()` 内で自己同期し、ホストは
    // 関与しない（ADR-0069 完成、#474）。golden frame の IME 面はアダプタ側（`ime_bridge.rs`
    // ユニット + `edit_context_browser.rs` 契約テスト）が担うため、ここでは `imeBounds` を
    // 取らずツリー/スタイル/レイアウト/a11y のクロスシームのみをスナップショットする。
    const source = fixture.raw as unknown as GoldenFrameSource;
    const frame = captureGoldenFrame(source, 1, null);

    expect(frame.elements.find((el) => el.id === inputId)?.textContent).toBe('Hi!');
    expect(frame).toMatchSnapshot();
  });
});
