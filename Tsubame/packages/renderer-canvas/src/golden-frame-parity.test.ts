import { afterEach, describe, expect, it } from 'vitest';
import {
  findElementByText,
  mountGoldenFrameParity,
  type GoldenFrameParityHarness,
} from './test-helpers/golden-frame-parity-harness.js';

describe('golden frame semantic parity (ADR-0079, #151)', () => {
  let harness: GoldenFrameParityHarness | null = null;

  afterEach(() => {
    harness?.dispose();
    harness = null;
  });

  it('defaultColor on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultColor: '#ff6600',
      });
      setText(text, 'ambient');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'ambient');
    expect(text?.visual?.textColor?.r).toBe(1);
    expect(text?.visual?.textColor?.g).toBeCloseTo(102 / 255, 5);
    expect(text?.visual?.textColor?.b).toBe(0);
    expect(frame).toMatchSnapshot();
  });

  it('block box color and fontSize do not leak to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        color: '#ff0000',
        fontSize: 24,
      });
      setText(text, 'child');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'child');
    expect(text?.visual?.textColor).toEqual({ r: 0, g: 0, b: 0, a: 1 });
    expect(text?.visual?.fontSize).toBe(16);
    expect(frame).toMatchSnapshot();
  });

  it('text-local color and fontSize apply on the text element', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { color: '#00ff00', fontSize: 20 });
      setText(text, 'styled');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'styled');
    expect(text?.visual?.textColor?.g).toBe(1);
    expect(text?.visual?.fontSize).toBe(20);
    expect(frame).toMatchSnapshot();
  });

  it('nested text elements inherit parent text styles in IFC', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const outer = createElement('text');
      const inner = createElement('text');
      insertNode(view, outer);
      insertNode(outer, inner);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        color: '#ff0000',
        fontSize: 32,
      });
      setProp(outer, 'style', { color: '#336699', fontSize: 18 });
      setText(outer, 'Hi ');
      setText(inner, 'there');
      return view;
    });

    const frame = harness.capture();
    const inner = findElementByText(frame, 'there');
    expect(inner?.visual?.fontSize).toBe(18);
    expect(inner?.visual?.textColor?.b).toBeCloseTo(153 / 255, 5);
    expect(frame).toMatchSnapshot();
  });

  it('fontWeight 600 resolves on text for variable-font synthesis', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { fontWeight: 600, fontSize: 24 });
      setText(text, 'w600');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'w600');
    expect(text?.visual?.fontWeight).toBe(600);
    expect(frame).toMatchSnapshot();
  });

  it('fontStyle italic resolves on text for faux-italic synthesis', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { fontStyle: 'italic', fontSize: 24 });
      setText(text, 'italic');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'italic');
    expect(text?.visual?.fontStyle).toBe('italic');
    expect(frame).toMatchSnapshot();
  });

  it('fontWeight 700 resolves on text for bold synthesis', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', { width: '200px', height: '100px' });
      setProp(text, 'style', { fontWeight: 700, fontSize: 24 });
      setText(text, 'bold');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'bold');
    expect(text?.visual?.fontWeight).toBe(700);
    expect(frame).toMatchSnapshot();
  });

  it('flexWrap wrap places overflow flex children on a second row', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp }) => {
      const row = createElement('view');
      setProp(row, 'style', {
        display: 'flex',
        flexWrap: 'wrap',
        width: '70px',
        gap: 0,
      });
      for (let i = 0; i < 3; i++) {
        const child = createElement('view');
        insertNode(row, child);
        setProp(child, 'style', { width: '40px', height: '15px' });
      }
      return row;
    });

    const frame = harness.capture();
    const children = frame.elements
      .filter((el) => el.bounds[2] === 40 && el.bounds[3] === 15)
      .sort((a, b) => a.bounds[0]! - b.bounds[0]! || a.bounds[1]! - b.bounds[1]!);
    expect(children).toHaveLength(3);
    expect(children[2]!.bounds[1]).toBeGreaterThan(children[0]!.bounds[1]!);
    expect(frame).toMatchSnapshot();
  });

  it('defaultFontSize on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultFontSize: 22,
      });
      setText(text, 'sized');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'sized');
    expect(text?.visual?.fontSize).toBe(22);
    expect(frame).toMatchSnapshot();
  });

  it('defaultFontFamily on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultFontFamily: 'Noto Sans',
      });
      setText(text, 'family');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'family');
    expect(text?.visual?.fontFamily).toBe('Noto Sans');
    expect(frame).toMatchSnapshot();
  });

  it('defaultFontWeight on a block box penetrates to descendant text', async () => {
    harness = await mountGoldenFrameParity(({ createElement, insertNode, setProp, setText }) => {
      const view = createElement('view');
      const text = createElement('text');
      insertNode(view, text);
      setProp(view, 'style', {
        width: '200px',
        height: '100px',
        defaultFontWeight: 600,
      });
      setText(text, 'weighted');
      return view;
    });

    const frame = harness.capture();
    const text = findElementByText(frame, 'weighted');
    expect(text?.visual?.fontWeight).toBe(600);
    expect(frame).toMatchSnapshot();
  });
});
