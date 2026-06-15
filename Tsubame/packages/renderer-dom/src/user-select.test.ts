import { describe, expect, it, beforeEach } from 'vitest';
import { resolveUserSelect } from './user-select.js';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

describe('resolveUserSelect (ADR-0097 Selection Region → user-select)', () => {
  it('defaults to none when no selectable boundary is set', () => {
    expect(resolveUserSelect('view', undefined)).toBe('none');
  });

  it('maps a selectable boundary to text', () => {
    expect(resolveUserSelect('view', true)).toBe('text');
  });

  it('keeps an explicitly unselectable element at none', () => {
    expect(resolveUserSelect('view', false)).toBe('none');
  });

  it('keeps text-input selectable regardless of the boundary', () => {
    expect(resolveUserSelect('text-input', undefined)).toBe('text');
    expect(resolveUserSelect('text-input', false)).toBe('text');
  });
});

describe('DomRenderer user-select (ADR-0097 decision 5)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  it('defaults non-selectable elements to user-select: none', () => {
    const renderer = new DomRenderer({ document, container });
    const view = renderer.createElement('view');
    renderer.setRoot(view);
    expect(container.querySelector('div')!.style.userSelect).toBe('none');
  });

  it('opens a selectable view to native selection (user-select: text)', () => {
    const renderer = new DomRenderer({ document, container });
    const view = renderer.createElement('view');
    renderer.setRoot(view);
    renderer.setProperty(view, 'selectable', true);
    expect(container.querySelector('div')!.style.userSelect).toBe('text');
  });

  it('re-bounds a view back to none when selectable is cleared', () => {
    const renderer = new DomRenderer({ document, container });
    const view = renderer.createElement('view');
    renderer.setRoot(view);
    renderer.setProperty(view, 'selectable', true);
    renderer.setProperty(view, 'selectable', false);
    expect(container.querySelector('div')!.style.userSelect).toBe('none');
  });

  it('keeps a text-input selectable even outside any Selection Region', () => {
    const renderer = new DomRenderer({ document, container });
    const input = renderer.createElement('text-input');
    renderer.setRoot(input);
    expect(container.querySelector('input')!.style.userSelect).toBe('text');
    renderer.setProperty(input, 'selectable', false);
    expect(container.querySelector('input')!.style.userSelect).toBe('text');
  });
});
