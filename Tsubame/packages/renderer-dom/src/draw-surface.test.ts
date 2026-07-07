import { describe, expect, it } from 'vitest';
import {
  DRAW_OVERFLOW_VISIBLE_MARGIN_PX,
  drawSurfaceGeometry,
} from './draw-surface.js';

// #731: draw 用 <canvas> の敷き方。DPR 追従（物理解像度 = 論理サイズ ×
// devicePixelRatio・painter から不可視）と、overflow: visible（既定）の
// はみ出しに備えた余白確保（名前付き定数・マジックナンバー禁止）。

describe('drawSurfaceGeometry (#731)', () => {
  it('doubles the physical resolution at DPR 2.0 while css size stays logical', () => {
    const g = drawSurfaceGeometry({ width: 120, height: 80 }, 2, 'hidden');
    expect(g.cssWidth).toBe(120);
    expect(g.cssHeight).toBe(80);
    expect(g.deviceWidth).toBe(240);
    expect(g.deviceHeight).toBe(160);
  });

  it('over-allocates the canvas by the named visible-overflow margin on every side', () => {
    const g = drawSurfaceGeometry({ width: 100, height: 50 }, 1, 'visible');
    // box の外へ margin ぶん張り出す（負オフセット）ので、box 外の描画が切れない。
    expect(g.cssLeft).toBe(-DRAW_OVERFLOW_VISIBLE_MARGIN_PX);
    expect(g.cssTop).toBe(-DRAW_OVERFLOW_VISIBLE_MARGIN_PX);
    expect(g.cssWidth).toBe(100 + 2 * DRAW_OVERFLOW_VISIBLE_MARGIN_PX);
    expect(g.cssHeight).toBe(50 + 2 * DRAW_OVERFLOW_VISIBLE_MARGIN_PX);
    // painter の原点（box 左上）は margin ぶん内側。
    expect(g.originX).toBe(DRAW_OVERFLOW_VISIBLE_MARGIN_PX);
    expect(g.originY).toBe(DRAW_OVERFLOW_VISIBLE_MARGIN_PX);
  });

  it('allocates exactly the box under overflow: hidden (clipped at the box anyway)', () => {
    const g = drawSurfaceGeometry({ width: 100, height: 50 }, 1, 'hidden');
    expect(g.cssLeft).toBe(0);
    expect(g.cssTop).toBe(0);
    expect(g.cssWidth).toBe(100);
    expect(g.cssHeight).toBe(50);
    expect(g.originX).toBe(0);
    expect(g.originY).toBe(0);
  });

  it('applies DPR to the over-allocated size (margin scales into device pixels too)', () => {
    const g = drawSurfaceGeometry({ width: 100, height: 50 }, 2, 'visible');
    expect(g.deviceWidth).toBe((100 + 2 * DRAW_OVERFLOW_VISIBLE_MARGIN_PX) * 2);
    expect(g.deviceHeight).toBe((50 + 2 * DRAW_OVERFLOW_VISIBLE_MARGIN_PX) * 2);
  });

  it('rounds fractional device sizes up so the surface never falls short of a pixel', () => {
    const g = drawSurfaceGeometry({ width: 101, height: 51 }, 1.5, 'hidden');
    expect(g.deviceWidth).toBe(Math.ceil(101 * 1.5));
    expect(g.deviceHeight).toBe(Math.ceil(51 * 1.5));
  });
});
