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

  it('swaps a text-input to a <textarea> when multiline is set, preserving the value (#362)', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('text-input');
    renderer.setRoot(id);
    renderer.setProperty(id, 'value', 'line one');
    expect(container.querySelector('input')).not.toBeNull();

    renderer.setProperty(id, 'multiline', true);

    expect(container.querySelector('input')).toBeNull();
    const textarea = container.querySelector('textarea');
    expect(textarea).not.toBeNull();
    expect(textarea!.value).toBe('line one', 'the live value carries across the swap');
    expect(textarea!.getAttribute('data-tsubame-id')).toBe(String(id as unknown as number));
  });

  it('swaps a multiline text-input back to an <input> when multiline is cleared (#362)', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('text-input');
    renderer.setRoot(id);
    renderer.setProperty(id, 'multiline', true);
    expect(container.querySelector('textarea')).not.toBeNull();

    renderer.setProperty(id, 'multiline', false);

    expect(container.querySelector('textarea')).toBeNull();
    expect(container.querySelector('input')).not.toBeNull();
  });

  it('keeps event listeners working across a multiline swap (#362)', () => {
    const renderer = new DomRenderer({ document, container });
    const id = renderer.createElement('text-input');
    renderer.setRoot(id);
    let fired = 0;
    renderer.addEventListener(id, 'input', () => {
      fired += 1;
    });

    renderer.setProperty(id, 'multiline', true);
    const textarea = container.querySelector('textarea')!;
    textarea.dispatchEvent(new (textarea.ownerDocument.defaultView as Window).Event('input'));

    expect(fired).toBe(1, 'the listener re-binds to the swapped-in textarea');
  });

  it('reflects the shared coerceElementProperty payload to the DOM (issue #235)', () => {
    // DOM 側は Canvas 側と同じ coerce 後のエッジケースを読まねばならない。
    // 両レンダラーは coerceElementProperty を経由するので、ここの反映は
    // 共有シームと完全に一致する。
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
