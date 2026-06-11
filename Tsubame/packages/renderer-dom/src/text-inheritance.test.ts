import { describe, it, expect, beforeEach } from 'vitest';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

describe('DOM Renderer two-channel text inheritance (ADR-0002)', () => {
  let document: Document;
  let container: HTMLElement;
  let window: import('happy-dom').Window;

  beforeEach(() => {
    ({ document, container, window } = createHappyDomFixture());
  });

  function computedColor(el: Element): string {
    return window.getComputedStyle(el as never).color;
  }

  function computedFontSize(el: Element): string {
    return window.getComputedStyle(el as never).fontSize;
  }

  it('does not emit view color or fontSize to block or descendant inline styles', () => {
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.appendChild(root, text);
    renderer.setRoot(root);
    renderer.setText(text, 'child');
    renderer.setStyle(root, { color: '#ff0000', fontSize: 24 });

    const viewEl = container.querySelector('div')!;
    const textEl = container.querySelector('span')!;
    expect(viewEl.style.color).toBe('');
    expect(viewEl.style.fontSize).toBe('');
    expect(textEl.style.color).toBe('');
    expect(textEl.style.fontSize).toBe('');
    expect(computedColor(textEl)).not.toBe('rgb(255, 0, 0)');
    expect(computedFontSize(textEl)).not.toBe('24px');
  });

  it('applies text-local styles directly on text elements', () => {
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.appendChild(root, text);
    renderer.setRoot(root);
    renderer.setText(text, 'styled');
    renderer.setStyle(text, { color: '#00ff00', fontSize: 20 });

    const textEl = container.querySelector('span')!;
    expect(textEl.style.color).toBe('#00ff00');
    expect(textEl.style.fontSize).toBe('20px');
  });

  it('emits default-* on block boxes so descendants inherit via CSS', () => {
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.appendChild(root, text);
    renderer.setRoot(root);
    renderer.setText(text, 'ambient');
    renderer.setStyle(root, {
      defaultColor: '#ff6600',
      defaultFontSize: 22,
      defaultFontWeight: 700,
    });

    const viewEl = container.querySelector('div')!;
    const textEl = container.querySelector('span')!;
    expect(viewEl.style.color).toBe('#ff6600');
    expect(viewEl.style.fontSize).toBe('22px');
    expect(viewEl.style.fontWeight).toBe('700');
    expect(textEl.style.color).toBe('');
    expect(computedColor(textEl)).toBe('#ff6600');
    expect(computedFontSize(textEl)).toBe('22px');
  });

  it('lets nested text elements inherit parent text styles in IFC', () => {
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const outer = renderer.createElement('text');
    const inner = renderer.createElement('text');
    renderer.appendChild(root, outer);
    renderer.setText(outer, 'Hi ');
    renderer.appendChild(outer, inner);
    renderer.setText(inner, 'there');
    renderer.setRoot(root);
    renderer.setStyle(outer, { fontSize: 18, color: '#336699' });
    renderer.setStyle(root, { fontSize: 32, color: '#ff0000' });

    const spans = container.querySelectorAll('span');
    const outerEl = spans[0]!;
    const innerEl = spans[1]!;
    expect(outerEl.style.fontSize).toBe('18px');
    expect(innerEl.style.fontSize).toBe('');
    expect(computedFontSize(innerEl)).toBe('18px');
    expect(computedColor(innerEl)).toBe('#336699');
  });

  it('applies fontStyle on text elements for DOM rendering', () => {
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const text = renderer.createElement('text');
    renderer.appendChild(root, text);
    renderer.setRoot(root);
    renderer.setText(text, 'slant');
    renderer.setStyle(text, { fontStyle: 'italic' });

    const textEl = container.querySelector('span')!;
    expect(textEl.style.fontStyle).toBe('italic');
  });

  it('applies text-local styles on text-input for its own field text', () => {
    const renderer = new DomRenderer({ document, container });
    const input = renderer.createElement('text-input');
    renderer.setRoot(input);
    renderer.setStyle(input, { color: '#abcdef', fontSize: 15 });

    const inputEl = container.querySelector('input')!;
    expect(inputEl.style.color).toBe('#abcdef');
    expect(inputEl.style.fontSize).toBe('15px');
  });
});
