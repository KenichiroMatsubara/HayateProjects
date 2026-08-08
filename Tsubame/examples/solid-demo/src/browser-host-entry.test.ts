// @vitest-environment happy-dom

import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { beforeEach, describe, expect, it } from 'vitest';

beforeEach(() => {
  document.body.innerHTML =
    '<div id="dom-host" hidden></div><canvas id="canvas-stage" hidden></canvas>';
  window.history.replaceState(null, '', '/?renderer=dom');
});

describe('Solid browser entry', () => {
  it('supplies both surfaces to Browser Host and keeps only the Solid mount seam', async () => {
    const entry = await import('./main');

    const dom = document.getElementById('dom-host') as HTMLDivElement;
    const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;
    await expect(entry.appHandle.settled).resolves.toEqual({ status: 'mounted' });
    expect(dom.hidden).toBe(false);
    expect(canvas.hidden).toBe(true);
    expect(dom.querySelector('[data-tsubame-id]')).not.toBeNull();

    const source = readFileSync(resolve(process.cwd(), 'src/main.tsx'), 'utf8');
    expect(source).not.toContain('shouldUseDomRenderer');
    expect(source).not.toContain('useDomRenderer');
    expect(source).not.toContain('DomRenderer');
    expect(source).not.toContain('HayateRenderer');

    entry.appHandle.dispose();
    expect(dom.hidden).toBe(true);
    expect(canvas.hidden).toBe(true);
  });
});
