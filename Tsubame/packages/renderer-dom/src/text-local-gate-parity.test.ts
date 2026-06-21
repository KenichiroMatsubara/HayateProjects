import { describe, it, expect, beforeEach } from 'vitest';
import type { ElementKind } from '@tsubame/renderer-protocol';
import { withTextLocalGate, carriesTextLocal } from '@tsubame/renderer-protocol';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

// 構造ベースの Semantics Parity（Tsubame ADR-0008）。Style Channel ゲートは
// 各 renderer の内部ではなく、すべての renderer の手前のシーム
// （`withTextLocalGate`）で一度だけ走る。本テストは実際の DOM renderer を
// 本番のシーム経由で駆動し、channel-1 の text-local プロパティが
// text-local を担う種別のときに限り要素へ届くことを検証する。各 renderer は
// 同一のゲート済みパッチを受け取るため、Canvas とのパリティは構造的に保たれる。

const ALL_KINDS: readonly ElementKind[] = [
  'view',
  'text',
  'image',
  'button',
  'text-input',
  'scroll-view',
];

describe('text-local gate through the seam (DOM renderer, Tsubame ADR-0008, #323)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  for (const kind of ALL_KINDS) {
    it(`${kind}: keeps text-local color iff the kind carries text-local`, () => {
      const renderer = withTextLocalGate(new DomRenderer({ document, container }));
      const id = renderer.createElement(kind);
      renderer.setRoot(id);
      renderer.setStyle(id, { color: '#ff0000', width: '100px' });

      const el = container.querySelector(`[data-tsubame-id="${id as number}"]`) as HTMLElement;
      // text-local でないプロパティは常に適用される。
      expect(el.style.width).toBe('100px');
      // text-local プロパティは Text-Local Carrier の種別でのみ適用される。
      if (carriesTextLocal(kind)) {
        expect(el.style.color).toBe('#ff0000');
      } else {
        expect(el.style.color).toBe('');
      }
    });
  }
});
