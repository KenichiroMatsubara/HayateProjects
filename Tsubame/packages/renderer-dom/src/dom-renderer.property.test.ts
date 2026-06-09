import { describe, it, expect, beforeEach } from 'vitest';
import { Window } from 'happy-dom';
import { DomRenderer } from './dom-renderer.js';

describe('DomRenderer setProperty (ADR-0071)', () => {
  let window: Window;
  let container: HTMLElement;

  beforeEach(() => {
    window = new Window();
    container = window.document.createElement('div');
    window.document.body.appendChild(container);
  });

  it('throws on unknown property names', () => {
    const renderer = new DomRenderer({ document: window.document, container });
    const id = renderer.createElement('view');
    expect(() => renderer.setProperty(id, 'data-foo', 'bar')).toThrow(
      /Unknown element property/,
    );
    expect(() => renderer.setProperty(id, 'aria-label', 'x')).toThrow(
      /Unknown element property/,
    );
  });

  it('applies value and placeholder on text-input', () => {
    const renderer = new DomRenderer({ document: window.document, container });
    const id = renderer.createElement('text-input');
    renderer.setRoot(id);
    renderer.setProperty(id, 'value', 'hello');
    renderer.setProperty(id, 'placeholder', 'type here');
    const el = container.querySelector('input')!;
    expect(el.value).toBe('hello');
    expect(el.placeholder).toBe('type here');
  });

  it('applies disabled and src on supported elements', () => {
    const renderer = new DomRenderer({ document: window.document, container });
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
});
