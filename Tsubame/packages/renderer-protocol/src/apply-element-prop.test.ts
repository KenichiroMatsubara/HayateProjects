import { describe, it, expect } from 'vitest';
import { RecordingRenderer } from './recording-renderer.js';
import { applyElementProp, type PropTarget } from './apply-element-prop.js';
import { ELEMENT_PROPERTY_NAMES } from './property.js';
import type { ElementId, ElementKind, Unsubscribe } from './index.js';

/**
 * 共通 `applyElementProp` seam を IRenderer 境界の記録だけで検証する（ADR-0008）。
 * solid / react の具象ハンドルに依らず、最小の `PropTarget` フェイクで dispatch ladder の
 * 各分岐を突く。
 */
function target(renderer: RecordingRenderer, kind: ElementKind): PropTarget & { id: ElementId } {
  const id = renderer.createElement(kind);
  const listeners = new Map<string, Unsubscribe>();
  return { id, kind, listeners };
}

describe('applyElementProp (shared Tsubame Adapter seam)', () => {
  it('splits style into base setStyle + per-pseudo setPseudoStyle', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'view');
    applyElementProp(r, t, 'style', {
      backgroundColor: '#fff',
      ':hover': { backgroundColor: '#eee' },
    });
    expect(r.styleOf(t.id)).toEqual({ backgroundColor: '#fff' });
    expect(r.calls).toContainEqual({
      method: 'setPseudoStyle',
      id: t.id,
      pseudo: ':hover',
      style: { backgroundColor: '#eee' },
    });
  });

  it('routes each styleVariants entry through setStyleVariant', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'view');
    const condition = { minWidth: 600 } as never;
    applyElementProp(r, t, 'styleVariants', [{ condition, style: { color: '#111' } }]);
    expect(r.calls).toContainEqual({
      method: 'setStyleVariant',
      id: t.id,
      condition,
      style: { color: '#111' },
    });
  });

  it('applies style to text but ignores non-style props on text', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'text');
    applyElementProp(r, t, 'style', { color: '#111' });
    applyElementProp(r, t, 'onClick', () => {});
    expect(r.styleOf(t.id)).toEqual({ color: '#111' });
    expect(r.calls.some((c) => c.method === 'addEventListener')).toBe(false);
  });

  it('subscribes an event prop, and swaps the subscription on replacement', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'button');
    applyElementProp(r, t, 'onClick', () => {});
    applyElementProp(r, t, 'onClick', () => {});
    const kinds = r.calls.map((c) => c.method);
    // 2回目で旧購読を解除してから張り直す。
    expect(kinds).toEqual([
      'createElement',
      'addEventListener',
      'removeEventListener',
      'addEventListener',
    ]);
  });

  it('unsubscribes an event prop when the handler is removed (non-function)', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'button');
    applyElementProp(r, t, 'onClick', () => {});
    applyElementProp(r, t, 'onClick', undefined);
    expect(r.calls.filter((c) => c.method === 'removeEventListener')).toHaveLength(1);
    expect(t.listeners.has('onClick')).toBe(false);
  });

  it('throws on a rejected event prop without recording a call', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'view');
    expect(() => applyElementProp(r, t, 'onHoverEnter', () => {})).toThrow(
      /onHoverEnter is not supported/,
    );
    expect(r.calls.some((c) => c.method === 'addEventListener')).toBe(false);
  });

  it('routes the draw prop to setDraw, and clears it with null on removal (#730)', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'view');
    const painter = () => {};
    applyElementProp(r, t, 'draw', painter);
    expect(r.calls).toContainEqual({ method: 'setDraw', id: t.id, value: painter });

    // prop 削除（undefined / null）は null に正規化して描画を消す。
    applyElementProp(r, t, 'draw', undefined);
    expect(r.calls).toContainEqual({ method: 'setDraw', id: t.id, value: null });
  });

  it('ignores structural props (children / ref / key)', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'view');
    applyElementProp(r, t, 'children', [{}]);
    applyElementProp(r, t, 'ref', () => {});
    applyElementProp(r, t, 'key', 'k');
    // createElement 以外の記録は無い。
    expect(r.calls.filter((c) => c.method !== 'createElement')).toHaveLength(0);
  });

  it('throws on an unknown element property', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'view');
    expect(() => applyElementProp(r, t, 'id', 'x')).toThrow(/Unknown element property "id"/);
  });

  it('forwards a known element property to setProperty', () => {
    const r = new RecordingRenderer();
    const t = target(r, 'text-input');
    const known = ELEMENT_PROPERTY_NAMES[0]!;
    applyElementProp(r, t, known, 'v');
    expect(r.calls).toContainEqual({
      method: 'setProperty',
      id: t.id,
      name: known,
      value: 'v',
    });
  });
});
