import { describe, it, expect } from 'vitest';
import type { ElementKind } from './element.js';
import type { StylePatch } from './style.js';
import { withTextLocalGate } from './gating-renderer.js';
import { RecordingRenderer } from './recording-renderer.js';

// Style Channel ゲートは seam で一度だけ、どの renderer よりも前に適用される
// （ADR-0008）。テストは in-memory の RecordingRenderer 越しに seam を駆動し、
// 記録された呼び出しを IRenderer インターフェース経由でのみ読む（private 状態は見ない）。

describe('text-local gate seam (Tsubame ADR-0008, #323)', () => {
  it('drops a text-local prop for a non-carrier kind before the inner renderer sees it', () => {
    const inner = new RecordingRenderer();
    const gate = withTextLocalGate(inner);

    const view = gate.createElement('view');
    gate.setStyle(view, { color: '#ff0000', width: '100px' });

    expect(inner.styleOf(view)).toEqual({ width: '100px' });
  });

  it('keeps text-local props for a carrier kind', () => {
    const inner = new RecordingRenderer();
    const gate = withTextLocalGate(inner);

    const text = gate.createElement('text');
    gate.setStyle(text, { color: '#ff0000', fontSize: 20, width: '100px' });

    expect(inner.styleOf(text)).toEqual({ color: '#ff0000', fontSize: 20, width: '100px' });
  });

  it('gates pseudo-style and viewport-variant patches the same way', () => {
    const inner = new RecordingRenderer();
    const gate = withTextLocalGate(inner);

    const view = gate.createElement('view');
    gate.setPseudoStyle(view, ':hover', { color: '#ff0000', backgroundColor: '#00ff00' });
    gate.setStyleVariant(view, { minWidth: 768 }, { fontSize: 18, width: '50px' });

    const pseudo = inner.calls.find((c) => c.method === 'setPseudoStyle');
    const variant = inner.calls.find((c) => c.method === 'setStyleVariant');
    expect(pseudo).toMatchObject({ style: { backgroundColor: '#00ff00' } });
    expect(variant).toMatchObject({ style: { width: '50px' } });
    // text-local プロパティは両方から除外されている
    expect(pseudo && 'style' in pseudo && pseudo.style).not.toHaveProperty('color');
    expect(variant && 'style' in variant && variant.style).not.toHaveProperty('fontSize');
  });

  it('forwards non-style ops verbatim', () => {
    const inner = new RecordingRenderer();
    const gate = withTextLocalGate(inner);

    const root = gate.createElement('view');
    const child = gate.createElement('text');
    gate.setRoot(root);
    gate.appendChild(root, child);
    gate.setText(child, 'hi');
    gate.setProperty(child, 'text-content', 'hi');
    gate.resize(800, 600);

    expect(inner.calls.map((c) => c.method)).toEqual([
      'createElement',
      'createElement',
      'setRoot',
      'appendChild',
      'setText',
      'setProperty',
      'resize',
    ]);
  });

  it('passes a patch through untouched when the element kind is unknown', () => {
    const inner = new RecordingRenderer();
    const gate = withTextLocalGate(inner);

    // id 999 は seam 経由で生成されていないため kind が不明
    const untracked = 999 as ReturnType<RecordingRenderer['createElement']>;
    gate.setStyle(untracked, { color: '#ff0000', width: '100px' });

    expect(inner.styleOf(untracked)).toEqual({ color: '#ff0000', width: '100px' });
  });

  it('hands every renderer behind the seam the identical filtered patch (parity by construction)', () => {
    const ALL_KINDS: readonly ElementKind[] = [
      'view',
      'text',
      'image',
      'button',
      'text-input',
      'scroll-view',
    ];
    const patch: StylePatch = {
      color: '#ff0000',
      fontSize: 16,
      backgroundColor: '#00ff00',
      width: '10px',
    };

    for (const kind of ALL_KINDS) {
      const a = new RecordingRenderer();
      const b = new RecordingRenderer();
      const gateA = withTextLocalGate(a);
      const gateB = withTextLocalGate(b);

      const idA = gateA.createElement(kind);
      const idB = gateB.createElement(kind);
      gateA.setStyle(idA, patch);
      gateB.setStyle(idB, patch);

      expect(a.styleOf(idA)).toEqual(b.styleOf(idB));
    }
  });
});
