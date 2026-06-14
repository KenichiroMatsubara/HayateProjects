import { describe, it, expect, beforeEach } from 'vitest';
import { coerceElementProperty } from '@tsubame/renderer-protocol';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

describe('DomRenderer setProperty (ADR-0071)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  it('throws on unknown property names', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('view');
    expect(() => renderer.setProperty(id, 'data-foo', 'bar')).toThrow(
      /Unknown element property/,
    );
    expect(() => renderer.setProperty(id, 'aria-label', 'x')).toThrow(
      /Unknown element property/,
    );
  });

  it('applies value and placeholder on text-input', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('text-input');
    renderer.setRoot(id);
    renderer.setProperty(id, 'value', 'hello');
    renderer.setProperty(id, 'placeholder', 'type here');
    const el = container.querySelector('input')!;
    expect(el.value).toBe('hello');
    expect(el.placeholder).toBe('type here');
  });

  it('applies disabled and src on supported elements', () => {
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const button = renderer.createElement('button');
    const image = renderer.createElement('image');
    renderer.appendChild(root, button);
    renderer.appendChild(root, image);
    renderer.setRoot(root);
    renderer.setProperty(button, 'disabled', true);
    expect(container.querySelector('button')!.disabled).toBe(true);

    renderer.setProperty(image, 'src', 'https://example.com/a.png');
    expect(container.querySelector('img')!.getAttribute('src')).toBe(
      'https://example.com/a.png',
    );
  });

  it('reflects the shared coerceElementProperty payload to the DOM (issue #235)', () => {
    // The DOM side must read the *same* coerced edge cases as the Canvas side —
    // both renderers route through coerceElementProperty, so the reflection here
    // matches the shared seam exactly.
    const renderer = new DomRenderer({ document, container });
    const root = renderer.createElement('view');
    const input = renderer.createElement('text-input');
    const button = renderer.createElement('button');
    renderer.appendChild(root, input);
    renderer.appendChild(root, button);
    renderer.setRoot(root);

    renderer.setProperty(input, 'value', 42);
    expect(container.querySelector('input')!.value).toBe(
      (coerceElementProperty('value', 42) as { text: string }).text,
    );

    renderer.setProperty(input, 'placeholder', 99);
    expect(container.querySelector('input')!.placeholder).toBe(
      (coerceElementProperty('placeholder', 99) as { text: string }).text,
    );

    renderer.setProperty(button, 'disabled', 'false');
    expect(container.querySelector('button')!.disabled).toBe(
      (coerceElementProperty('disabled', 'false') as { disabled: boolean }).disabled,
    );
  });
});
