import { describe, it, expect, beforeEach } from 'vitest';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

describe('StylePatch declaration emitter parity', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  function pseudoRuleBody(renderer: DomRenderer, id: ReturnType<DomRenderer['createElement']>): string {
    const styleEl = document.querySelector('style[data-tsubame-pseudo]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const rule = styleEl.sheet.cssRules[0] as unknown as CSSStyleRule;
    return rule.style.cssText;
  }

  function variantRuleBody(renderer: DomRenderer, id: ReturnType<DomRenderer['createElement']>): string {
    const styleEl = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    const mediaRule = styleEl.sheet.cssRules[0] as unknown as CSSMediaRule;
    const inner = mediaRule.cssRules[0] as unknown as CSSStyleRule;
    return inner.style.cssText;
  }

  it('base setStyle applies border-style:solid when borderWidth is positive', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyle(id, { borderWidth: 2 });

    const el = container.querySelector('div')!;
    expect(el.style.borderWidth).toBe('2px');
    expect(el.style.borderStyle).toBe('solid');
  });

  it(':hover { borderWidth } emits border-style:solid in pseudo rule body', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setPseudoStyle(id, ':hover', { borderWidth: 2 });

    const body = pseudoRuleBody(renderer, id);
    expect(body).toContain('border-width: 2px');
    expect(body).toContain('border-style: solid');
  });

  it('viewport variant { borderWidth } emits border-style:solid in rule body', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { borderWidth: 2 });

    const body = variantRuleBody(renderer, id);
    expect(body).toContain('border-width: 2px');
    expect(body).toContain('border-style: solid');
  });

  it('skips channel-1 text-local keys on block boxes for all three paths', () => {
    const renderer = new DomRenderer({ document, container });
    const viewId = renderer.createElement('view');
    const textId = renderer.createElement('text');
    renderer.appendChild(viewId, textId);
    renderer.setRoot(viewId);

    renderer.setStyle(viewId, { color: '#ff0000', fontSize: 24 });
    renderer.setPseudoStyle(viewId, ':hover', { color: '#00ff00', fontSize: 20 });
    renderer.setStyleVariant(viewId, { minWidth: 768 }, { color: '#0000ff', fontSize: 18 });

    const viewEl = container.querySelector('div')!;
    expect(viewEl.style.color).toBe('');
    expect(viewEl.style.fontSize).toBe('');

    const pseudoSheet = document.querySelector('style[data-tsubame-pseudo]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    expect(pseudoSheet.sheet.cssRules.length).toBe(0);

    const variantSheet = document.querySelector('style[data-tsubame-variant]')! as unknown as {
      sheet: CSSStyleSheet;
    };
    expect(variantSheet.sheet.cssRules.length).toBe(0);
  });

  it('applies channel-1 text-local keys on text elements for all three paths', () => {
    const renderer = new DomRenderer({ document, container });
    const rootId = renderer.createElement('view');
    const textId = renderer.createElement('text');
    renderer.appendChild(rootId, textId);
    renderer.setRoot(rootId);
    renderer.setText(textId, 'styled');

    renderer.setStyle(textId, { color: '#00ff00', fontSize: 20 });
    renderer.setPseudoStyle(textId, ':hover', { color: '#ff0000' });
    renderer.setStyleVariant(textId, { minWidth: 768 }, { fontSize: 22 });

    const textEl = container.querySelector('span')!;
    expect(textEl.style.color).toBe('#00ff00');
    expect(textEl.style.fontSize).toBe('20px');

    const pseudoBody = pseudoRuleBody(renderer, textId);
    expect(pseudoBody).toContain('color: #ff0000');

    const variantBody = variantRuleBody(renderer, textId);
    expect(variantBody).toContain('font-size: 22px');
  });
});
