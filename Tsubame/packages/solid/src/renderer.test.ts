import { describe, it, expect, vi } from 'vitest';
import { isTextInTextCollapse, createElementNode } from './node.js';

describe('isTextInTextCollapse', () => {
  it('is true for text child of text parent (DOM span model)', () => {
    const parent = createElementNode(1 as never, 'text');
    const child = createElementNode(2 as never, 'text', 'Hi');
    expect(isTextInTextCollapse(parent, child)).toBe(true);
  });

  it('is false for text child of button (ADR-0058)', () => {
    const parent = createElementNode(1 as never, 'button');
    const child = createElementNode(2 as never, 'text', 'Go');
    expect(isTextInTextCollapse(parent, child)).toBe(false);
  });
});

describe('renderer integration (stub IRenderer)', () => {
  it('insertNode collapses text-in-text and destroys orphan id', async () => {
    const { insertNode, createElement, createTextNode } = await import('./renderer.js');
    const { setActiveRenderer } = await import('./active-renderer.js');

    const ops: string[] = [];
    let nextId = 0;
    const stub = {
      createElement: (kind: string) => {
        nextId += 1;
        ops.push(`create:${kind}`);
        return nextId as never;
      },
      setRoot: () => {},
      appendChild: (p: number, c: number) => ops.push(`append:${p},${c}`),
      insertBefore: (p: number, c: number, b: number) =>
        ops.push(`insert:${p},${c},${b}`),
      removeChild: (_p: number, c: number) => ops.push(`remove:${c}`),
      setStyle: () => {},
      setText: (id: number, text: string) => ops.push(`text:${id}=${text}`),
      setProperty: () => {},
      addEventListener: () => () => {},
      resize: () => {},
    };

    setActiveRenderer(stub as never);
    const outer = createElement('text');
    const inner = createTextNode('TS');
    insertNode(outer, inner);

    expect(ops).toContain('create:text');
    expect(ops).toContain('text:2=TS');
    expect(ops).toContain('text:1=TS');
    expect(ops).toContain('remove:2');
    expect(ops.some((o) => o.startsWith('append:'))).toBe(false);
  });

  it('insertNode appends text under button', async () => {
    const { insertNode, createElement, createTextNode } = await import('./renderer.js');
    const { setActiveRenderer } = await import('./active-renderer.js');

    const ops: string[] = [];
    let nextId = 0;
    const stub = {
      createElement: (kind: string) => {
        nextId += 1;
        ops.push(`create:${kind}`);
        return nextId as never;
      },
      setRoot: () => {},
      appendChild: (p: number, c: number) => ops.push(`append:${p},${c}`),
      insertBefore: () => {},
      removeChild: () => {},
      setStyle: () => {},
      setText: (id: number, text: string) => ops.push(`text:${id}=${text}`),
      setProperty: () => {},
      addEventListener: () => () => {},
      resize: () => {},
    };

    setActiveRenderer(stub as never);
    const button = createElement('button');
    const label = createTextNode('OK');
    insertNode(button, label);

    expect(ops).toContain('append:1,2');
  });

  it('rejects onHoverEnter (ADR-0059)', async () => {
    const { setProp, createElement } = await import('./renderer.js');
    const { setActiveRenderer } = await import('./active-renderer.js');

    setActiveRenderer({
      createElement: () => 1 as never,
      setRoot: () => {},
      appendChild: () => {},
      insertBefore: () => {},
      removeChild: () => {},
      setStyle: () => {},
      setText: () => {},
      setProperty: () => {},
      addEventListener: () => () => {},
      resize: () => {},
    } as never);

    const view = createElement('view');
    expect(() => setProp(view, 'onHoverEnter', () => {})).toThrow(
      /onHoverEnter is not supported/,
    );
  });
});
