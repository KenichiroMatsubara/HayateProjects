import { describe, it, expect, vi } from 'vitest';
import { EVENT_KIND, OP, TAG, USER_SELECT } from '@torimi/tsubame-protocol-generated/protocol';
import { coerceElementProperty, withTextLocalGate } from '@torimi/tsubame-renderer-protocol';
import { HayateRenderer } from './hayate-renderer.js';
import { StubHayate, manualScheduler } from './test-helpers/stub-hayate.js';

// 構築≠開始：host-blind コアは clock（requestFrame/cancelFrame）だけを受け取り、
// frame ループは明示 start() でしか走らない（#476, ADR-0004）。
describe('HayateRenderer lifecycle (host-blind core, #476)', () => {
  it('does not run the frame loop until start() (no constructor side effects)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });

    // 構築だけでは frame は登録されない。tick しても render は走らない。
    sched.tick(16);
    expect(hayate.renders).toEqual([]);

    // start() で初めてループが武装し、tick で flush→render→poll が走る。
    renderer.start();
    sched.tick(16);
    expect(hayate.renders).toEqual([16]);

    // stop() で停止し、以後の tick では走らない。
    renderer.stop();
    sched.tick(32);
    expect(hayate.renders).toEqual([16]);
  });
});

// ADR-0126: フレームループは App Host の idle/wake 契約に従う。描画と次フレームの
// スケジューリングを分離し、継続 pending があるときだけ再武装する。idle では 1 枚も出さず、
// signal 変化（mutation 到着）で冷間始動する。
describe('HayateRenderer on-demand frame loop (ADR-0126, #608)', () => {
  it('does not reschedule when no visual work is pending (idle → 0 further frames)', () => {
    const hayate = new StubHayate();
    hayate.pendingVisualWork = false;
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    // 最初のフレームは走る（start の冷間始動）。
    sched.tick(16);
    expect(hayate.renders).toEqual([16]);

    // idle（visual_dirty 空）：ループは再武装しない。以降の tick は no-op。
    sched.tick(32);
    sched.tick(48);
    expect(hayate.renders).toEqual([16]);
  });

  it('keeps requesting frames while visual work is pending, and stops when it clears', () => {
    const hayate = new StubHayate();
    hayate.pendingVisualWork = true;
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    // 進行中の transition / カーソル点滅 / スクロール物理：毎フレーム継続要求（退行なし）。
    sched.tick(16);
    sched.tick(32);
    sched.tick(48);
    expect(hayate.renders).toEqual([16, 32, 48]);

    // pending が解消したフレームは描画し、その後は再武装しない（idle へ落ちる）。
    hayate.pendingVisualWork = false;
    sched.tick(64);
    expect(hayate.renders).toEqual([16, 32, 48, 64]);
    sched.tick(80);
    expect(hayate.renders).toEqual([16, 32, 48, 64]);
  });

  it('cold-starts the idle loop when a mutation arrives (signal-change wake)', () => {
    const hayate = new StubHayate();
    hayate.pendingVisualWork = false;
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    sched.tick(16); // 初回フレーム → idle
    sched.tick(32); // idle: no-op
    expect(hayate.renders).toEqual([16]);

    // consumer の signal 変化が mutation を積む → idle ループを冷間始動する。
    renderer.createElement('view');
    sched.tick(48);
    expect(hayate.renders).toEqual([16, 48]);
    // その mutation はこのフレームで flush された。
    expect(hayate.mutations.length).toBeGreaterThan(0);
  });

  it('wakes the idle loop when input arrives (ADR-0080/0126 input wake; Android Chrome tap fix)', () => {
    // 回帰: ADR-0126 の on-demand ループ化で web の入力到着 wake が配線漏れになり、
    // idle 時のタップが drain されず捨てられていた（Android Chrome でボタンが無反応）。
    // Platform Adapter（自前配線ポインタ listener）の request_redraw が idle ループを
    // 冷間始動しなければならない。
    const hayate = new StubHayate();
    hayate.pendingVisualWork = false;
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    // 冷間始動フレームの後、idle に落ちる（継続 pending なし）。
    sched.tick(16);
    sched.tick(32);
    expect(hayate.renders).toEqual([16]);

    // renderer は start() で入力 wake を adapter に配線していなければならない。
    expect(hayate.requestRedraw).toBeTypeOf('function');

    // 入力到着（タップ）を模す: adapter の listener がバッファ後に wake を叩く。
    // これで idle ループが 1 フレームだけ起き、pending_pointer が drain される。
    hayate.requestRedraw?.();
    sched.tick(48);
    expect(hayate.renders).toEqual([16, 48]);
  });

  it('does not arm the loop from a mutation before start() (構築≠開始)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });

    // start 前の mutation はループを武装しない。tick しても描画は走らない。
    renderer.createElement('view');
    sched.tick(16);
    expect(hayate.renders).toEqual([]);
  });
});

// 配信ポーリングのみ。apply_mutations のワイヤ統合は wasm-integration.test.ts にある。
describe('HayateRenderer delivery poll (ADR-0053)', () => {
  it('flushes delivery-handler mutations before committing the prepared frame (#827)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
    const button = renderer.createElement('button');
    const label = renderer.createElement('text');
    renderer.addEventListener(button, 'click', () => renderer.setText(label, 'clicked'));
    hayate.events = [[1, EVENT_KIND.CLICK, button, 0, 0]];

    sched.tick(16);

    expect(hayate.mutations.flatMap((batch) => batch.texts)).toContain('clicked');
    expect(hayate.committedFrames).toEqual([1]);
  });

  it('aborts a prepared frame and discards its mutation packet when a handler throws (#827)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
    const button = renderer.createElement('button');
    const label = renderer.createElement('text');
    renderer.addEventListener(button, 'click', () => {
      renderer.setText(label, 'must-not-leak');
      throw new Error('handler failed');
    });
    hayate.events = [[1, EVENT_KIND.CLICK, button, 0, 0]];

    expect(() => sched.tick(16)).toThrow('handler failed');
    expect(hayate.abortedFrames).toEqual([1]);
    expect(hayate.committedFrames).toEqual([]);

    hayate.events = [];
    renderer.start();
    sched.tick(32);
    expect(hayate.mutations.flatMap((batch) => batch.texts)).not.toContain('must-not-leak');
  });

  it('aborts a prepared frame and discards queued mutations when apply_mutations fails', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
    const failed = renderer.createElement('text');
    renderer.setText(failed, 'must-not-leak');
    const apply = hayate.apply_mutations.bind(hayate);
    hayate.apply_mutations = (ops, styles, texts, draws) => {
      apply(ops, styles, texts, draws);
      throw new Error('apply failed');
    };

    expect(() => sched.tick(16)).toThrow('apply failed');
    expect(hayate.abortedFrames).toEqual([1]);
    expect(hayate.committedFrames).toEqual([]);

    hayate.apply_mutations = apply;
    const next = renderer.createElement('text');
    renderer.setText(next, 'next-frame');
    sched.tick(32);

    expect(hayate.mutations.at(-1)?.texts).toEqual(['next-frame']);
    expect(hayate.committedFrames).toEqual([2]);
  });

  it('snapshots a style patch when it is enqueued', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
    const view = renderer.createElement('view');
    const patch = { opacity: 0.2 };

    renderer.setStyle(view, patch);
    patch.opacity = 0.8;
    sched.tick(16);

    const batch = hayate.mutations[0]!;
    expect(batch.styles[0]).toBe(TAG.OPACITY);
    expect(batch.styles[1]).toBeCloseTo(0.2);
  });

  it('registers Hayate listeners and dispatches poll deliveries', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

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

  it('delivers captured pointer coordinates and kind through the renderer seam', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

    const canvas = renderer.createElement('view');
    const received: unknown[] = [];
    renderer.addEventListener(canvas, 'pointermove', (event) => received.push(event));

    hayate.events = [[1, EVENT_KIND.POINTER_DRAG, canvas, 30, 40, 1]];
    sched.tick();

    expect(received).toEqual([
      { kind: 'pointermove', target: canvas, x: 30, y: 40, pointerKind: 1 },
    ]);
  });

  it('removeChild requires adapter unsubscribe before stale deliveries stop', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();

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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
    const id = renderer.createElement('view');
    expect(() => renderer.setProperty(id, 'className', 'x')).toThrow(
      /Unknown element property/,
    );
  });

  it('routes known setProperty names to Hayate (ADR-0071)', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
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
      const renderer = new HayateRenderer({ raw: hayate, ...sched });
      renderer.start();
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
      } else if (expected.kind === 'multiline') {
        expect(batch.ops[at + 2]).toBe(expected.multiline ? 1 : 0);
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
    const inner = new HayateRenderer({ raw: hayate, ...sched });
    inner.start();
    const renderer = withTextLocalGate(inner);
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
    const inner = new HayateRenderer({ raw: hayate, ...sched });
    inner.start();
    const renderer = withTextLocalGate(inner);
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
    const inner = new HayateRenderer({ raw: hayate, ...sched });
    inner.start();
    const renderer = withTextLocalGate(inner);
    const view = renderer.createElement('view');

    renderer.setPseudoStyle(view, ':hover', { color: '#ff0000', fontSize: 18 });
    sched.tick();

    const batch = hayate.mutations[0]!;
    expect(batch.ops).not.toContain(OP.SET_PSEUDO_STYLE);
  });

  it('unsubscribe stops delivery dispatch', () => {
    const hayate = new StubHayate();
    const sched = manualScheduler();
    const renderer = new HayateRenderer({ raw: hayate, ...sched });
    renderer.start();
    const node = renderer.createElement('button');

    const handler = vi.fn();
    const unsub = renderer.addEventListener(node, 'click', handler);
    unsub();

    hayate.events = [[1, 0, 1, 0, 0]];
    sched.tick();
    expect(handler).not.toHaveBeenCalled();
  });
});
