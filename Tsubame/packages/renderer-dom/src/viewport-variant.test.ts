import { describe, it, expect, beforeEach } from 'vitest';
import { Window } from 'happy-dom';
import { DomRenderer } from './dom-renderer.js';

describe('DomRenderer setStyleVariant (ADR-0081)', () => {
  let window: Window;
  let container: HTMLElement;

  beforeEach(() => {
    window = new Window();
    container = window.document.createElement('div');
    window.document.body.appendChild(container);
  });

  it('emits an @media (min-width: ...) rule for the element', () => {
    const renderer = new DomRenderer({ document: window.document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#0000ff' });

    const styleEl = window.document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    expect(mediaRule.conditionText).toBe('(min-width: 768px)');
    const inner = mediaRule.cssRules[0] as unknown as CSSStyleRule;
    expect(inner.selectorText).toBe(`[data-tsubame-id="${id as unknown as number}"]`);
    expect(inner.style.backgroundColor).toBeTruthy();
  });

  it('combines multiple condition axes with "and"', () => {
    const renderer = new DomRenderer({ document: window.document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(
      id,
      { minWidth: 768, maxWidth: 1024 },
      { backgroundColor: '#00ff00' },
    );

    const styleEl = window.document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    expect(mediaRule.conditionText).toBe('(min-width: 768px) and (max-width: 1024px)');
  });

  it('updates an existing variant rule in place', () => {
    const renderer = new DomRenderer({ document: window.document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#0000ff' });
    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#ff0000' });

    const styleEl = window.document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    expect(styleEl.sheet.cssRules.length).toBe(1);
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    const inner = mediaRule.cssRules[0] as unknown as CSSStyleRule;
    expect(inner.style.backgroundColor).toBe('#ff0000');
  });
});
