import { describe, it, expect } from 'vitest';

describe('renderer integration (stub IRenderer)', () => {
  it('insertNode appends text under text parent (IFC inline element)', async () => {
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
    expect(ops).toContain('append:1,2');
    expect(ops).not.toContain('text:1=TS');
    expect(ops).not.toContain('remove:2');
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

  it('rejects unknown element properties (ADR-0071)', async () => {
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
    expect(() => setProp(view, 'id', 'x')).toThrow(/Unknown element property/);
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
