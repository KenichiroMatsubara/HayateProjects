import { describe, expect, it, beforeEach } from 'vitest';
import { resolveUserSelect } from './user-select.js';
import { DomRenderer } from './dom-renderer.js';
import { createHappyDomFixture } from './test-helpers/happy-dom-fixture.js';

describe('resolveUserSelect (ADR-0108 kind default + explicit user-select)', () => {
  it('defaults a view to text (selectable by element-kind default)', () => {
    expect(resolveUserSelect('view', undefined)).toBe('text');
  });

  it('defaults a button to none (kind default excludes it)', () => {
    expect(resolveUserSelect('button', undefined)).toBe('none');
  });

  it('lets an explicit none exclude an otherwise-selectable view', () => {
    expect(resolveUserSelect('view', 'none')).toBe('none');
  });

  it('lets an explicit text override a button kind default', () => {
    expect(resolveUserSelect('button', 'text')).toBe('text');
  });

  it('treats contains as selectable (CSS text; boundary resolved core-side)', () => {
    expect(resolveUserSelect('view', 'contains')).toBe('text');
  });

  it('keeps text-input selectable regardless of explicit value', () => {
    expect(resolveUserSelect('text-input', undefined)).toBe('text');
    expect(resolveUserSelect('text-input', 'none')).toBe('text');
  });
});

describe('DomRenderer user-select (ADR-0108)', () => {
  let document: Document;
  let container: HTMLElement;

  beforeEach(() => {
    ({ document, container } = createHappyDomFixture());
  });

  it('defaults a view to user-select: text (selectable by kind default)', () => {
    const renderer = new DomRenderer({ document, container });
    const view = renderer.createElement('view');
    renderer.setRoot(view);
    expect(container.querySelector('div')!.style.userSelect).toBe('text');
  });

  it('defaults a button to user-select: none (kind default excludes it)', () => {
    const renderer = new DomRenderer({ document, container });
    const button = renderer.createElement('button');
    renderer.setRoot(button);
    expect(container.querySelector('button')!.style.userSelect).toBe('none');
  });

  it('excludes a view from selection on user-select: none', () => {
    const renderer = new DomRenderer({ document, container });
    const view = renderer.createElement('view');
    renderer.setRoot(view);
    renderer.setProperty(view, 'user-select', 'none');
    expect(container.querySelector('div')!.style.userSelect).toBe('none');
  });

  it('re-opens a view to text when user-select returns to text', () => {
    const renderer = new DomRenderer({ document, container });
    const view = renderer.createElement('view');
    renderer.setRoot(view);
    renderer.setProperty(view, 'user-select', 'none');
    renderer.setProperty(view, 'user-select', 'text');
    expect(container.querySelector('div')!.style.userSelect).toBe('text');
  });

  it('keeps a text-input selectable regardless of an explicit user-select', () => {
    const renderer = new DomRenderer({ document, container });
    const input = renderer.createElement('text-input');
    renderer.setRoot(input);
    expect(container.querySelector('input')!.style.userSelect).toBe('text');
    renderer.setProperty(input, 'user-select', 'none');
    expect(container.querySelector('input')!.style.userSelect).toBe('text');
  });
});
