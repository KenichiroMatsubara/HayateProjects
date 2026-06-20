import { describe, it, expect, beforeEach } from 'vitest';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

describe('DomRenderer setStyleVariant (ADR-0081)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  it('emits an @media (min-width: ...) rule for the element', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#0000ff' });

    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    expect(mediaRule.conditionText).toBe('(min-width: 768px)');
    const inner = mediaRule.cssRules[0] as unknown as CSSStyleRule;
    expect(inner.selectorText).toBe(`[data-tsubame-id="${id as unknown as number}"]`);
    expect(inner.style.backgroundColor).toBeTruthy();
  });

  it('marks variant declarations !important so they override the inline base style', () => {
    // ベーススタイルは el.style（インライン）に載るため、@media variant が
    // ベースを上書きするには !important が必須（インラインは通常規則より優先）。
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    // ベースで display:flex を敷き、狭幅 variant で none に上書きする。
    renderer.setStyle(id, { display: 'flex' });
    renderer.setStyleVariant(id, { maxWidth: 719 }, { display: 'none' });

    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    const inner = mediaRule.cssRules[0] as unknown as CSSStyleRule;
    expect(inner.style.getPropertyPriority('display')).toBe('important');
  });

  it('combines multiple condition axes with "and"', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(
      id,
      { minWidth: 768, maxWidth: 1024 },
      { backgroundColor: '#00ff00' },
    );

    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    expect(mediaRule.conditionText).toBe('(min-width: 768px) and (max-width: 1024px)');
  });

  it('emits all four viewport axes in the media query', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(
      id,
      { minWidth: 768, maxWidth: 1024, minHeight: 600, maxHeight: 900 },
      { backgroundColor: '#0000ff' },
    );

    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    expect(mediaRule.conditionText).toBe(
      '(min-width: 768px) and (max-width: 1024px) and (min-height: 600px) and (max-height: 900px)',
    );
  });

  it('keeps multiple variant rules in declaration order', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#0000ff' });
    renderer.setStyleVariant(id, { minWidth: 1024 }, { backgroundColor: '#00ff00' });

    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    expect(styleEl.sheet.cssRules.length).toBe(2);
    const first = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    const second = styleEl.sheet.cssRules[1] as unknown as CSSMediaRule;
    expect(first.conditionText).toBe('(min-width: 768px)');
    expect(second.conditionText).toBe('(min-width: 1024px)');
  });

  it('updates an existing variant rule in place', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#0000ff' });
    renderer.setStyleVariant(id, { minWidth: 768 }, { backgroundColor: '#ff0000' });

    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    expect(styleEl.sheet.cssRules.length).toBe(1);
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    const inner = mediaRule.cssRules[0] as unknown as CSSStyleRule;
    expect(inner.style.backgroundColor).toBe('#ff0000');
  });
});
