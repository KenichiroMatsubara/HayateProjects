import { describe, it, expect, beforeEach } from 'vitest';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

function pseudoRules(document: Document): CSSStyleRule[] {
  const styleEl = document.querySelector('style[data-tsubame-pseudo]')! as unknown as {
    sheet: CSSStyleSheet;
  };
  return [...styleEl.sheet.cssRules] as unknown as CSSStyleRule[];
}

function pseudoBand(selectorText: string): number {
  if (selectorText.endsWith(':focus')) return 0;
  if (selectorText.endsWith(':hover')) return 1;
  if (selectorText.endsWith(':active')) return 2;
  throw new Error(`unknown pseudo selector: ${selectorText}`);
}

describe('DomRenderer pseudo-state priority (focus < hover < active)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  it('inserts pseudo rules in priority-band order regardless of call order', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setPseudoStyle(id, ':active', { backgroundColor: '#0000ff' });
    renderer.setPseudoStyle(id, ':hover', { backgroundColor: '#00ff00' });
    renderer.setPseudoStyle(id, ':focus', { backgroundColor: '#ff0000' });

    const bands = pseudoRules(document).map((rule) => pseudoBand(rule.selectorText));
    expect(bands).toEqual([0, 1, 2]);
  });

  it('keeps active rule after hover when hover is authored last', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setPseudoStyle(id, ':active', { backgroundColor: '#0000ff' });
    renderer.setPseudoStyle(id, ':hover', { backgroundColor: '#00ff00' });

    const rules = pseudoRules(document);
    const hoverIdx = rules.findIndex((r) => r.selectorText.endsWith(':hover'));
    const activeIdx = rules.findIndex((r) => r.selectorText.endsWith(':active'));
    expect(activeIdx).toBeGreaterThan(hoverIdx);
  });
});
