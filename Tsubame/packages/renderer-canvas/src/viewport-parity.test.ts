// @vitest-environment happy-dom
//
// DOM/Canvas の viewport 条件パリティ＆Canvas viewport のリサイズ追従の回帰テスト（#429）。
//
// 診断（#429）が live に測れなかった一点 ── DOM レンダラと Canvas レンダラが、同じ
// 幅で同じ `styleVariant`（`maxWidth`/`minWidth`）を「適用/非適用」一致させるか ── を
// CI で検査可能な不変条件に落とす。ヘッドレスブラウザは無いので、両経路の *解決入力* を
// 突き合わせる:
//
//   - DOM 経路: `DomRenderer` が実際に発行した `@media` 規則を取り出し、ブラウザの
//     `matchMedia` で当該幅に対する適用状態を読む。
//   - Canvas 経路: 解決は Hayate（Rust 側 `ViewportCondition.matches`、閉区間）が行うが、
//     その判定入力 ── レンダラが Hayate に転送した viewport 幅と、エンコードした閾値 ──
//     だけが TS 層の責務。ここを `CanvasRenderer` の実リサイズ経路（ResizeObserver →
//     `resize` → `on_resize`）で固定する。
//
// これにより、ResizeObserver の取りこぼしで Canvas viewport が既定（800×600）のまま
// 据え置かれ、狭幅で DOM が隠した要素（例: 優先度ラベル）が Canvas にだけ現れる乖離を
// 回帰として検出できる。

import { describe, it, expect } from 'vitest';
import { Window } from 'happy-dom';
import { DomRenderer } from '@tsubame/renderer-dom';
import { OP } from '@tsubame/protocol-generated/protocol';
import { CanvasRenderer } from './canvas-renderer.js';
import { manualScheduler, type ManualScheduler } from './test-helpers/manual-scheduler.js';
import type { RawHayate } from './hayate.js';

/** 全テストで共有する breakpoint。CSS の `(max-width: 719px)` に対応する。 */
const MAX_WIDTH = 719;
const VIEWPORT_HEIGHT = 600;
/** Hayate `ElementTree::new()` の既定 viewport（ADR-0081）。リサイズ取りこぼし時に
 * Canvas viewport が据え置かれる値。 */
const DEFAULT_WIDTH = 800;

// ── DOM 経路 ────────────────────────────────────────────────────────────────

/** `DomRenderer` に `maxWidth:719` variant を発行させ、その実 `@media` 規則を
 * `width` で評価した適用状態を返す（DOM 解決そのもの）。 */
function domVariantApplies(width: number): boolean {
  const window = new Window();
  // happy-dom の DOM 型は lib.dom と構造的に互換でないため、この境界でキャストする
  // （renderer-dom の happy-dom fixture と同じ扱い）。
  const containerNode = window.document.createElement('div');
  window.document.body.appendChild(containerNode);
  const document = window.document as unknown as Document;
  const container = containerNode as unknown as HTMLElement;

  const renderer = new DomRenderer({ document, container });
  const id = renderer.createElement('view');
  renderer.setRoot(id);
  renderer.setStyle(id, { display: 'flex' });
  renderer.setStyleVariant(id, { maxWidth: MAX_WIDTH }, { display: 'none' });

  const styleEl = document.querySelector('style[data-tsubame-variant]') as unknown as {
    sheet: CSSStyleSheet;
  };
  const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;

  window.innerWidth = width;
  return window.matchMedia(mediaRule.conditionText).matches;
}

// ── Canvas 経路 ─────────────────────────────────────────────────────────────

/** `on_resize`（viewport 転送）と `apply_mutations`（variant エンコード）だけを記録する
 * 最小 Hayate スタブ。解決自体は Rust 側にあり、ここでは TS 層が転送した入力を観測する。 */
class RecordingHayate {
  resizes: Array<{ width: number; height: number; scale: number }> = [];
  mutations: Array<{ ops: number[] }> = [];

  on_resize(width: number, height: number, scale: number): void {
    this.resizes.push({ width, height, scale });
  }
  apply_mutations(ops: Float64Array): void {
    this.mutations.push({ ops: Array.from(ops) });
  }
  render(): void {}
  poll_events(): unknown[] {
    return [];
  }
  ime_wants_keyboard(): boolean {
    return false;
  }
}

class MockResizeObserver {
  static instances: MockResizeObserver[] = [];
  constructor(private readonly callback: ResizeObserverCallback) {
    MockResizeObserver.instances.push(this);
  }
  observe(): void {}
  disconnect(): void {}
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
  return {
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
  } as unknown as HTMLCanvasElement;
}

interface CanvasHarness {
  hayate: RecordingHayate;
  renderer: CanvasRenderer;
  sched: ManualScheduler;
  observer: MockResizeObserver;
}

/** 既定（800×600）から開始する `CanvasRenderer` をマウントする。構築時の初期同期で
 * 800×600 が転送される（既定 viewport を模す）。 */
function mountCanvas(): CanvasHarness {
  const hayate = new RecordingHayate();
  const sched = manualScheduler();
  MockResizeObserver.instances = [];
  const canvas = createCanvas(DEFAULT_WIDTH, VIEWPORT_HEIGHT);
  const renderer = new CanvasRenderer(hayate as unknown as RawHayate, {
    ...sched,
    canvas,
    devicePixelRatio: 1,
    createResizeObserver: MockResizeObserver as unknown as typeof ResizeObserver,
  });
  return { hayate, renderer, sched, observer: MockResizeObserver.instances[0]! };
}

/** `CanvasRenderer` がパケットへエンコードした variant の maxWidth 軸を読み出す。 */
function canvasEncodedMaxWidth(harness: CanvasHarness): number {
  const view = harness.renderer.createElement('view');
  harness.renderer.setStyleVariant(view, { maxWidth: MAX_WIDTH }, { display: 'none' });
  harness.sched.tick();
  const batch = harness.hayate.mutations.at(-1)!;
  const i = batch.ops.indexOf(OP.SET_STYLE_VARIANT);
  // op レイアウト（ADR-0081）: [op, id, minWidth, maxWidth, minHeight, maxHeight, offset, len]
  return batch.ops[i + 3]!;
}

/** Canvas が Hayate へ転送中の viewport 幅（最後の `on_resize`）。 */
function canvasTrackedWidth(harness: CanvasHarness): number {
  return harness.hayate.resizes.at(-1)!.width;
}

/** ADR-0081 の閉区間セマンティクス: `maxWidth` は `actual <= max` で一致する
 * （CSS の `(max-width: …)` に倣う）。Hayate の解決と同じ判定を、Canvas が
 * 転送した viewport 幅とエンコードした閾値に対して適用する。 */
function canvasVariantApplies(trackedWidth: number, encodedMaxWidth: number): boolean {
  return trackedWidth <= encodedMaxWidth;
}

describe('DOM/Canvas viewport-condition parity & Canvas resize tracking (#429)', () => {
  it('a maxWidth:719 variant resolves to the same applied state on DOM and Canvas at a given width', () => {
    // 狭幅（変種が効く）と広幅（効かない）の両側で、両経路の解決が一致すること。
    for (const width of [700, 800]) {
      const harness = mountCanvas();
      const encodedMaxWidth = canvasEncodedMaxWidth(harness);
      harness.observer.emit(width, VIEWPORT_HEIGHT);

      const canvasApplies = canvasVariantApplies(
        canvasTrackedWidth(harness),
        encodedMaxWidth,
      );
      const domApplies = domVariantApplies(width);

      expect(canvasApplies, `parity at width ${width}`).toBe(domApplies);
      harness.renderer.stop();
    }
  });

  it('the Canvas viewport tracks resize and does not stay at the 800×600 default', () => {
    const harness = mountCanvas();
    // 構築時の初期同期で既定 800×600 が転送される。
    expect(harness.hayate.resizes.at(-1)).toEqual({
      width: DEFAULT_WIDTH,
      height: VIEWPORT_HEIGHT,
      scale: 1,
    });

    // 狭幅へリサイズすると、その新サイズが Hayate へ転送されること。
    harness.observer.emit(700, VIEWPORT_HEIGHT);

    expect(harness.hayate.resizes.at(-1)).toEqual({
      width: 700,
      height: VIEWPORT_HEIGHT,
      scale: 1,
    });
    expect(canvasTrackedWidth(harness), 'viewport must not stay at the default').not.toBe(
      DEFAULT_WIDTH,
    );
    harness.renderer.stop();
  });

  it('parity breaks if the Canvas viewport is left stale at the default after a resize', () => {
    // 実 viewport は 700px（狭幅）。DOM はこれで variant を適用する。
    const realWidth = 700;
    expect(domVariantApplies(realWidth)).toBe(true);

    const harness = mountCanvas();
    const encodedMaxWidth = canvasEncodedMaxWidth(harness);
    harness.observer.emit(realWidth, VIEWPORT_HEIGHT);

    // 据え置かれた既定 viewport（800）では variant が効かず、DOM と乖離する
    // ── これが回帰として検出したい不具合。
    const staleApplies = canvasVariantApplies(DEFAULT_WIDTH, encodedMaxWidth);
    expect(staleApplies, 'a stale 800px viewport would diverge from DOM').toBe(false);

    // 追従できている viewport（700）ではパリティが回復する。
    const liveApplies = canvasVariantApplies(canvasTrackedWidth(harness), encodedMaxWidth);
    expect(liveApplies, 'a live viewport restores parity with DOM').toBe(true);
    harness.renderer.stop();
  });
});
