import { describe, it, expect, beforeEach } from 'vitest';
import { withTextLocalGate } from '@tsubame/renderer-protocol';
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

  it('borderWidth alone does not imply a border-style (declarative model, #204)', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyle(id, { borderWidth: 2 });

    const el = container.querySelector('div')!;
    expect(el.style.borderWidth).toBe('2px');
    // border-style は独立したプロパティ。width が 'solid' を暗黙に決めることはない。
    expect(el.style.borderStyle).not.toBe('solid');
  });

  it('base setStyle maps borderStyle:dashed to CSS border-style:dashed', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyle(id, { borderWidth: 2, borderStyle: 'dashed' });

    const el = container.querySelector('div')!;
    expect(el.style.borderWidth).toBe('2px');
    expect(el.style.borderStyle).toBe('dashed');
  });

  it(':hover { borderStyle } emits border-style in the pseudo rule body', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setPseudoStyle(id, ':hover', { borderStyle: 'dashed' });

    const body = pseudoRuleBody(renderer, id);
    expect(body).toContain('border-style: dashed');
  });

  it('viewport variant { borderStyle } emits border-style in the rule body', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyleVariant(id, { minWidth: 768 }, { borderStyle: 'solid' });

    const body = variantRuleBody(renderer, id);
    expect(body).toContain('border-style: solid');
  });

  it('maps a box-shadow list to a CSS box-shadow string (#252)', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyle(id, {
      boxShadow: [
        { offsetX: 0, offsetY: 4, blur: 8, spread: 0, color: '#00000080', inset: false },
        { offsetX: 0, offsetY: 0, blur: 0, spread: 3, color: '#1e90ff', inset: true },
      ],
    });

    const el = container.querySelector('div')!;
    expect(el.style.boxShadow).toBe(
      '0px 4px 8px 0px #00000080, inset 0px 0px 0px 3px #1e90ff',
    );
  });

  it('maps an empty box-shadow list to CSS none', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyle(id, { boxShadow: [] });

    const el = container.querySelector('div')!;
    expect(el.style.boxShadow).toBe('none');
  });

  it('maps position + insets to CSS for absolute positioning (#205)', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    renderer.setRoot(id);

    renderer.setStyle(id, { position: 'absolute', top: 10, left: 20, right: 30, bottom: 40 });

    const el = container.querySelector('div')!;
    expect(el.style.position).toBe('absolute');
    expect(el.style.top).toBe('10px');
    expect(el.style.left).toBe('20px');
    expect(el.style.right).toBe('30px');
    expect(el.style.bottom).toBe('40px');
  });

  it('skips channel-1 text-local keys on block boxes for all three paths', () => {
    // ゲーティングは seam の責務。DOM レンダラはその経由で駆動する。
    const renderer = withTextLocalGate(new DomRenderer({ document, container }));
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
