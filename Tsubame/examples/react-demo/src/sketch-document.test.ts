import { describe, expect, it } from 'vitest';
import { Canvas } from '@torimi/tsubame-protocol-generated/recorder';
import { SketchDocument } from './sketch-document.js';

function displayList(document: SketchDocument): number[] {
  const canvas = new Canvas();
  document.frame().paint(canvas, { width: 320, height: 480 });
  return canvas.finish();
}

describe('SketchDocument', () => {
  it('turns one pointer gesture into one continuous painted stroke', () => {
    const document = new SketchDocument();

    expect(document.begin({ x: 10, y: 20 })).toBe(true);
    expect(document.append({ x: 30, y: 40 })).toBe(true);
    expect(document.end({ x: 50, y: 60 })).toBe(true);

    expect(document.strokeCount).toBe(1);
    expect(document.isDrawing).toBe(false);
    expect(displayList(document).length).toBeGreaterThan(0);
  });

  it('undo removes exactly the most recently committed stroke', () => {
    const document = new SketchDocument();
    document.begin({ x: 10, y: 10 });
    document.end({ x: 20, y: 20 });
    document.begin({ x: 30, y: 30 });
    document.end({ x: 40, y: 40 });

    expect(document.undo()).toBe(true);
    expect(document.strokeCount).toBe(1);
    expect(document.undo()).toBe(true);
    expect(document.strokeCount).toBe(0);
    expect(document.undo()).toBe(false);
  });

  it('clear removes committed and in-progress drawing in one action', () => {
    const document = new SketchDocument();
    document.begin({ x: 10, y: 10 });
    document.end({ x: 20, y: 20 });
    document.begin({ x: 30, y: 30 });

    expect(document.clear()).toBe(true);
    expect(document.strokeCount).toBe(0);
    expect(document.isDrawing).toBe(false);
    expect(displayList(document)).toEqual([]);
    expect(document.clear()).toBe(false);
  });

  it('changes the brush width used by the next stroke', () => {
    const document = new SketchDocument();

    expect(document.strokeWidth).toBe(5);
    expect(document.setStrokeWidth(11)).toBe(true);
    expect(document.strokeWidth).toBe(11);
    expect(document.setStrokeWidth(11)).toBe(false);
  });
});
