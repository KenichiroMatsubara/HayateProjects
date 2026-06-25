import { describe, it, expect, vi } from 'vitest';
import { EVENT_KIND, OP, TAG, USER_SELECT } from '@tsubame/protocol-generated/protocol';
import { coerceElementProperty, withTextLocalGate } from '@tsubame/renderer-protocol';
import { CanvasRenderer } from './canvas-renderer.js';
import type { RawHayate } from './hayate.js';

class StubHayate implements RawHayate {
  mutations: Array<{ ops: number[]; styles: number[]; texts: string[] }> = [];
  renders: number[] = [];
  events: unknown[][] = [];
  listenerSeq = 1;
  registeredListeners: Array<{ elementId: number; eventKind: number; listenerId: number }> =
    [];
  textContentCalls: Array<[number, string]> = [];
  textCalls: Array<[number, string]> = [];
  srcCalls: Array<[number, string]> = [];
  disabledCalls: Array<[number, boolean]> = [];
  pseudoStyleCalls: Array<[number, number, number[]]> = [];

  element_create(): void {}
  set_root(): void {}
  element_set_text(id: number, text: string): void {
    this.textCalls.push([id, text]);
  }
  element_set_text_content(id: number, text: string): void {
    this.textContentCalls.push([id, text]);
  }
  element_set_src(id: number, url: string): void {
    this.srcCalls.push([id, url]);
  }
  element_set_disabled(id: number, disabled: boolean): void {
    this.disabledCalls.push([id, disabled]);
  }
  element_get_text(): string {
    return '';
  }
  element_append_child(): void {}
  element_insert_before(): void {}
  element_remove(): void {}
  element_subtree_ids(): Float64Array {
    return new Float64Array();
  }
  element_set_style(): void {}
  element_set_pseudo_style(id: number, state: number, packed: Float32Array): void {
    this.pseudoStyleCalls.push([id, state, Array.from(packed)]);
  }
  apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[]): void {
    this.mutations.push({
      ops: Array.from(ops),
      styles: Array.from(styles),
      texts: Array.from(texts),
    });
  }
  resizes: Array<{ width: number; height: number; scale: number }> = [];
  on_resize(width: number, height: number, scale: number): void {
    this.resizes.push({ width, height, scale });
  }
  on_pointer_move(): void {}
  on_pointer_down(): void {}
  on_pointer_up(): void {}
  on_wheel(): void {}
  on_key_down(): void {}
  has_selection(): boolean {
    return false;
  }
  on_text_input(): void {}
  // IME（on_composition_* / ime_* / focused_element_id / element_get_text_content）は
  // RawHayate ポートから外れた（IME 配線はアダプタ内で完結、#474）ため stub にも無い。
  element_get_bounds(): number[] {
    return [0, 0, 0, 0];
  }
  element_effective_visual(): null {
    return null;
  }
  poll_accessibility(): string | null {
    return null;
  }
  render(timestampMs: number): void {
    this.renders.push(timestampMs);
  }
  poll_events(): unknown[] {
    const current = this.events;
    this.events = [];
    return current;
  }
  register_listener(elementId: number, eventKind: number): number {
    const listenerId = this.listenerSeq++;
    this.registeredListeners.push({ elementId, eventKind, listenerId });
    return listenerId;
  }
  set_background_color(): void {}
  set_tuning(): void {}
}

function manualScheduler() {
  let pending: FrameRequestCallback | null = null;
  return {
    requestFrame: (cb: FrameRequestCallback) => {
      pending = cb;
      return 1;
    },
    cancelFrame: () => {
      pending = null;
    },
    tick: (timestamp = 16) => {
      const cb = pending;
      pending = null;
      cb?.(timestamp);
    },
  };
}

// 配信ポーリングのみ。apply_mutations のワイヤ統合は wasm-integration.test.ts にある。
describe('CanvasRenderer delivery poll (ADR-0053)', () => {
  it('registers Hayate listeners and dispatches poll deliveries', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);

    const button = renderer.createElement('button');
    const label = renderer.createElement('text');
    renderer.appendChild(button, label);

    const received: unknown[] = [];
    renderer.addEventListener(button, 'click', (event) => received.push(event));

    expect(hayate.registeredListeners).toEqual([
      { elementId: 1, eventKind: 0, listenerId: 1 },
    ]);

    hayate.events = [[1, 0, 2, 10, 20]];
    sched.tick();

    expect(received).toEqual([{ kind: 'click', target: 2 }]);
  });

  it('delivers the input event value straight from the wire (full current value, no read-back)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);

    const input = renderer.createElement('text-input');
    renderer.setRoot(input);

    const received: unknown[] = [];
    renderer.addEventListener(input, 'input', (event) => received.push(event));

    // core が text_input 配信に要素の現在値全体（display_text）を載せるため（ADR-0069 完成、
    // #474）、ホストは `element_get_text_content` の読み戻しをせずワイヤの値をそのまま配る。
    // InteractionEvent.value は契約上その要素の現在値で、DOM レンダラの `target.value` と一致する。
    hayate.events = [[1, EVENT_KIND.TEXT_INPUT, input, 'ab']];
    sched.tick();

    expect(received).toEqual([{ kind: 'input', target: input, value: 'ab' }]);
  });

  it('removeChild requires adapter unsubscribe before stale deliveries stop', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);

    const parent = renderer.createElement('view');
    const child = renderer.createElement('view');
    const grandchild = renderer.createElement('text');
    renderer.appendChild(parent, child);
    renderer.appendChild(child, grandchild);

    const handler = vi.fn();
    const unsub = renderer.addEventListener(grandchild, 'click', handler);
    renderer.removeChild(parent, child);
    unsub();

    hayate.events = [[1, 0, 3, 0, 0]];
    sched.tick();
    expect(handler).not.toHaveBeenCalled();
  });

  it('batches setStyleVariant through apply_mutations as OP_SET_STYLE_VARIANT (ADR-0081)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const view = renderer.createElement('view');

    renderer.setStyleVariant(view, { minWidth: 768 }, { backgroundColor: '#0000ff' });

    sched.tick();

    expect(hayate.mutations).toHaveLength(1);
    const batch = hayate.mutations[0]!;
    const opIndex = batch.ops.indexOf(OP.SET_STYLE_VARIANT);
    expect(opIndex).toBeGreaterThanOrEqual(0);
    expect(batch.ops[opIndex + 1]).toBe(view as unknown as number);
    expect(batch.ops[opIndex + 2]).toBe(768); // minWidth
    expect(batch.ops[opIndex + 3]).toBe(-1); // maxWidth（未設定、ADR-0081 のセンチネル）
    expect(batch.ops[opIndex + 4]).toBe(-1); // minHeight
    expect(batch.ops[opIndex + 5]).toBe(-1); // maxHeight
    expect(batch.styles.length).toBeGreaterThan(0);
  });

  it('batches setPseudoStyle through apply_mutations without element_set_pseudo_style', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const button = renderer.createElement('button');

    renderer.setPseudoStyle(button, ':hover', { backgroundColor: '#0000ff' });

    sched.tick();

    expect(hayate.pseudoStyleCalls).toHaveLength(0);
    expect(hayate.mutations).toHaveLength(1);
    const batch = hayate.mutations[0]!;
    expect(batch.ops).toContain(OP.SET_PSEUDO_STYLE);
    expect(batch.ops[batch.ops.indexOf(OP.SET_PSEUDO_STYLE) + 1]).toBe(1);
    expect(batch.ops[batch.ops.indexOf(OP.SET_PSEUDO_STYLE) + 2]).toBe(0);
    expect(batch.styles.length).toBeGreaterThan(0);
  });

  it('preserves pseudo-style, base style, and structure order in one batch', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const root = renderer.createElement('view');
    const button = renderer.createElement('button');
    renderer.setRoot(root);
    renderer.appendChild(root, button);
    renderer.setStyle(button, { backgroundColor: '#ffffff' });
    renderer.setPseudoStyle(button, ':hover', { backgroundColor: '#0000ff' });

    sched.tick();

    expect(hayate.pseudoStyleCalls).toHaveLength(0);
    expect(hayate.mutations).toHaveLength(1);
    const batch = hayate.mutations[0]!;
    const appendIdx = batch.ops.indexOf(OP.APPEND_CHILD);
    const setStyleIdx = batch.ops.indexOf(OP.SET_STYLE);
    const setPseudoIdx = batch.ops.indexOf(OP.SET_PSEUDO_STYLE);

    expect(appendIdx).toBeGreaterThanOrEqual(0);
    expect(setStyleIdx).toBeGreaterThan(appendIdx);
    expect(setPseudoIdx).toBeGreaterThan(setStyleIdx);
    expect(batch.ops[setPseudoIdx + 1]).toBe(2);
    expect(batch.ops[setPseudoIdx + 2]).toBe(0);
    expect(batch.styles.length).toBeGreaterThan(0);
  });

  it('batches setProperty with structure mutations in one apply_mutations', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const parent = renderer.createElement('view');
    const child = renderer.createElement('text-input');
    renderer.appendChild(parent, child);
    renderer.setProperty(child, 'value', 'typed');

    sched.tick();

    expect(hayate.mutations).toHaveLength(1);
    const batch = hayate.mutations[0]!;
    expect(batch.ops).toContain(OP.APPEND_CHILD);
    expect(batch.ops).toContain(OP.SET_TEXT_CONTENT);
    expect(batch.texts).toContain('typed');
  });

  it('defers setProperty value until frame flush via apply_mutations', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const input = renderer.createElement('text-input');

    renderer.setProperty(input, 'value', 'hi');

    expect(hayate.mutations).toHaveLength(0);
    expect(hayate.textContentCalls).toHaveLength(0);

    sched.tick();

    expect(hayate.textContentCalls).toHaveLength(0);
    expect(hayate.mutations).toHaveLength(1);
    const batch = hayate.mutations[0]!;
    expect(batch.texts).toContain('hi');
    const opIndex = batch.ops.indexOf(OP.SET_TEXT_CONTENT);
    expect(opIndex).toBeGreaterThanOrEqual(0);
    expect(batch.ops[opIndex + 1]).toBe(1);
    expect(batch.texts[batch.ops[opIndex + 2]!]).toBe('hi');
  });

  it('throws on unknown setProperty names (ADR-0071)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const id = renderer.createElement('view');
    expect(() => renderer.setProperty(id, 'className', 'x')).toThrow(
      /Unknown element property/,
    );
  });

  it('routes known setProperty names to Hayate (ADR-0071)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const input = renderer.createElement('text-input');
    const image = renderer.createElement('image');

    renderer.setProperty(input, 'value', 'hi');
    renderer.setProperty(input, 'placeholder', 'enter');
    renderer.setProperty(input, 'disabled', true);
    renderer.setProperty(image, 'src', 'https://example.com/x.png');
    sched.tick();

    const batch = hayate.mutations[0]!;
    expect(batch.texts).toContain('hi');
    expect(batch.ops).toContain(OP.SET_TEXT_CONTENT);
    expect(batch.texts).toContain('enter');
    expect(batch.ops).toContain(OP.SET_TEXT);
    expect(batch.ops).toContain(OP.SET_DISABLED);
    expect(batch.ops).toContain(OP.SET_SRC);
    expect(hayate.textCalls).toHaveLength(0);
    expect(hayate.disabledCalls).toHaveLength(0);
    expect(hayate.srcCalls).toHaveLength(0);
  });

  it('routes the multiline property to a SET_MULTILINE op (#362)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const input = renderer.createElement('text-input');

    renderer.setProperty(input, 'multiline', true);
    sched.tick();

    const batch = hayate.mutations[0]!;
    expect(batch.ops).toContain(OP.SET_MULTILINE);
  });

  it('applies the shared coerceElementProperty payload to the packet (issue #235)', () => {
    // 型強制の境界ケースを通し、パケットが共有シームの生成物どおりであることを確認する
    // （Canvas 側での再強制はない）。
    const cases: ReadonlyArray<[Parameters<typeof coerceElementProperty>[0], unknown, number]> = [
      ['value', 42, OP.SET_TEXT_CONTENT], // 数値は文字列化
      ['placeholder', 99, OP.SET_TEXT], // 非文字列は消去
      ['src', null, OP.SET_SRC], // null は消去
      ['disabled', 'false', OP.SET_DISABLED], // Boolean('false') === true
      ['user-select', 'none', OP.SET_USER_SELECT], // 閉じた語彙 → ワイヤ enum
    ];

    for (const [name, value, op] of cases) {
      const hayate = new StubHayate();
      const sched = manualScheduler();
      const renderer = new CanvasRenderer(hayate, sched);
      const el = renderer.createElement('text-input');
      renderer.setProperty(el, name, value);
      sched.tick();

      const batch = hayate.mutations[0]!;
      const at = batch.ops.indexOf(op);
      expect(at).toBeGreaterThanOrEqual(0);
      const expected = coerceElementProperty(name, value);
      if (expected.kind === 'disabled') {
        expect(batch.ops[at + 2]).toBe(expected.disabled ? 1 : 0);
      } else if (expected.kind === 'user-select') {
        expect(batch.ops[at + 2]).toBe(USER_SELECT[expected.value]);
      } else {
        expect(batch.texts[batch.ops[at + 2]!]).toBe(expected.text);
      }
    }
  });

  it('does not encode a text-local prop on a non-carrier kind (Tsubame ADR-0008, #323)', () => {
    // Style Channel ゲート: channel-1 の text-local プロパティ（ここでは `color`）は
    // Text-Local Carrier の種別にしか届かない。ゲートは Canvas レンダラの手前のシーム
    // （`withTextLocalGate`）で一度走るので、`view` の `color` はワイヤ手前で落とされ、
    // Hayate の lowering には委ねない。text-local でない `width` は通る。
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = withTextLocalGate(new CanvasRenderer(hayate, sched));
    const view = renderer.createElement('view');

    renderer.setStyle(view, { color: '#ff0000', width: '100px' });
    sched.tick();

    const batch = hayate.mutations[0]!;
    expect(batch.styles).not.toContain(TAG.COLOR);
    expect(batch.styles).toEqual([TAG.WIDTH, 100, 0]);
  });

  it('encodes text-local props on a carrier kind (Tsubame ADR-0008, #323)', () => {
    // `text` 要素は channel-1 の text-local プロパティを運ぶので、シームは
    // text-local でない `width` とともに `color` と `fontSize` を残す。
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = withTextLocalGate(new CanvasRenderer(hayate, sched));
    const text = renderer.createElement('text');

    renderer.setStyle(text, { color: '#ff0000', fontSize: 20, width: '100px' });
    sched.tick();

    const batch = hayate.mutations[0]!;
    expect(batch.styles).toContain(TAG.COLOR);
    expect(batch.styles).toContain(TAG.FONT_SIZE);
    expect(batch.styles).toContain(TAG.WIDTH);
  });

  it('gates text-local props out of a non-carrier pseudo-style before encode (#323)', () => {
    // シームのゲートはスタイルを伴う全 op で同一: 純粋に text-local なプロパティだけの
    // `view` の :hover パッチは空に潰れ、SET_PSEUDO_STYLE はワイヤに届かない。
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = withTextLocalGate(new CanvasRenderer(hayate, sched));
    const view = renderer.createElement('view');

    renderer.setPseudoStyle(view, ':hover', { color: '#ff0000', fontSize: 18 });
    sched.tick();

    const batch = hayate.mutations[0]!;
    expect(batch.ops).not.toContain(OP.SET_PSEUDO_STYLE);
  });

  it('unsubscribe stops delivery dispatch', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new CanvasRenderer(hayate, sched);
    const node = renderer.createElement('button');

    const handler = vi.fn();
    const unsub = renderer.addEventListener(node, 'click', handler);
    unsub();

    hayate.events = [[1, 0, 1, 0, 0]];
    sched.tick();
    expect(handler).not.toHaveBeenCalled();
  });
});

class MockResizeObserver {
  static instances: MockResizeObserver[] = [];
  readonly observed: Element[] = [];

  constructor(private readonly callback: ResizeObserverCallback) {
    MockResizeObserver.instances.push(this);
  }

  observe(target: Element): void {
    this.observed.push(target);
  }

  disconnect(): void {
    this.observed.length = 0;
  }

  emit(width: number, height: number): void {
    const contentRect = {
      width,
      height,
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      bottom: height,
      right: width,
      toJSON: () => ({}),
    };
    this.callback(
      [{ contentRect } as ResizeObserverEntry],
      this as unknown as ResizeObserver,
    );
  }
}

function createCanvas(cssWidth: number, cssHeight: number): HTMLCanvasElement {
  const canvas = {
    width: 0,
    height: 0,
    getBoundingClientRect: () => ({
      width: cssWidth,
      height: cssHeight,
      x: 0,
      y: 0,
      top: 0,
      left: 0,
      bottom: cssHeight,
      right: cssWidth,
      toJSON: () => ({}),
    }),
  };
  return canvas as unknown as HTMLCanvasElement;
}

function viewportOptions(
  canvas: HTMLCanvasElement,
  devicePixelRatio = 2,
): {
  canvas: HTMLCanvasElement;
  devicePixelRatio: number;
  createResizeObserver: typeof ResizeObserver;
} {
  MockResizeObserver.instances = [];
  return {
    canvas,
    devicePixelRatio,
    createResizeObserver: MockResizeObserver as unknown as typeof ResizeObserver,
  };
}

describe('CanvasRenderer viewport sizing (ADR-0007)', () => {
  it('observes the canvas and applies initial CSS layout size with devicePixelRatio', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const canvas = createCanvas(800, 600);
    const renderer = new CanvasRenderer(hayate, {
      ...sched,
      ...viewportOptions(canvas, 2),
    });

    expect(MockResizeObserver.instances).toHaveLength(1);
    expect(MockResizeObserver.instances[0]!.observed).toEqual([canvas]);
    expect(hayate.resizes).toEqual([{ width: 800, height: 600, scale: 2 }]);
    expect(canvas.width).toBe(1600);
    expect(canvas.height).toBe(1200);

    renderer.stop();
  });

  it('syncs the pixel buffer and notifies Hayate when the observed size changes', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const canvas = createCanvas(400, 300);
    const renderer = new CanvasRenderer(hayate, {
      ...sched,
      ...viewportOptions(canvas, 2),
    });

    MockResizeObserver.instances[0]!.emit(1024, 768);

    expect(hayate.resizes).toEqual([
      { width: 400, height: 300, scale: 2 },
      { width: 1024, height: 768, scale: 2 },
    ]);
    expect(canvas.width).toBe(2048);
    expect(canvas.height).toBe(1536);

    renderer.stop();
  });

  it('does not attach a ResizeObserver when autoResize is false', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const canvas = createCanvas(800, 600);
    const renderer = new CanvasRenderer(hayate, {
      ...sched,
      ...viewportOptions(canvas, 2),
      autoResize: false,
    });

    expect(MockResizeObserver.instances).toHaveLength(0);
    expect(hayate.resizes).toEqual([]);
    expect(canvas.width).toBe(0);
    expect(canvas.height).toBe(0);

    renderer.stop();
  });

  it('allows manual resize when autoResize is false', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const canvas = createCanvas(800, 600);
    const renderer = new CanvasRenderer(hayate, {
      ...sched,
      canvas,
      autoResize: false,
    });

    renderer.resize(640, 480, 3);

    expect(hayate.resizes).toEqual([{ width: 640, height: 480, scale: 3 }]);
    expect(canvas.width).toBe(1920);
    expect(canvas.height).toBe(1440);

    renderer.stop();
  });

  it('resizes with the live devicePixelRatio, not the value cached at construction', () => {
    // モバイル Chrome はレンダラ構築後に `devicePixelRatio` を変える。入力中に出る
    // ソフトキーボードやフォーカス時ズームが実効比率を変えるためだ。構築時にキャッシュ
    // した比率では backing store が小さく作られ、シーンが canvas に合わせて拡大されて
    // グリフが粗くなる。observer はリサイズごとに現在の比率を読む必要がある。
    const previous = globalThis.devicePixelRatio;
    try {
      (globalThis as { devicePixelRatio: number }).devicePixelRatio = 2;
      const hayate = new StubHayate();
      const sched = manualScheduler();
      const canvas = createCanvas(400, 300);
      // devicePixelRatio の明示指定なし → リサイズごとにグローバル値を追う必要がある。
      MockResizeObserver.instances = [];
      const renderer = new CanvasRenderer(hayate, {
        ...sched,
        canvas,
        createResizeObserver: MockResizeObserver as unknown as typeof ResizeObserver,
      });

      (globalThis as { devicePixelRatio: number }).devicePixelRatio = 3;
      MockResizeObserver.instances[0]!.emit(400, 300);

      const last = hayate.resizes.at(-1)!;
      expect(last).toEqual({ width: 400, height: 300, scale: 3 });
      expect(canvas.width).toBe(1200);
      expect(canvas.height).toBe(900);

      renderer.stop();
    } finally {
      (globalThis as { devicePixelRatio: number }).devicePixelRatio = previous;
    }
  });
});
