import { describe, it, expect } from 'vitest';
import { encodeStylePatch, unsetKindsOf, TAG, UNSET_KIND } from './style-encoder.js';

// ── encodeStylePatch ──────────────────────────────────────────────────────────

describe('encodeStylePatch – color properties', () => {
  it('encodes color as [TAG.COLOR, r, g, b, a]', () => {
    const out: number[] = [];
    encodeStylePatch({ color: '#ff0000' }, out);
    expect(out[0]).toBe(TAG.COLOR);
    expect(out).toHaveLength(5);
    expect(out[1]).toBeCloseTo(1, 5); // r
    expect(out[2]).toBeCloseTo(0, 5); // g
    expect(out[3]).toBeCloseTo(0, 5); // b
    expect(out[4]).toBeCloseTo(1, 5); // a
  });

  it('encodes backgroundColor', () => {
    const out: number[] = [];
    encodeStylePatch({ backgroundColor: 'rgba(0, 255, 0, 0.5)' }, out);
    expect(out[0]).toBe(TAG.BACKGROUND_COLOR);
    expect(out).toHaveLength(5);
    expect(out[1]).toBeCloseTo(0, 5);   // r
    expect(out[2]).toBeCloseTo(1, 5);   // g
    expect(out[3]).toBeCloseTo(0, 5);   // b
    expect(out[4]).toBeCloseTo(0.5, 3); // a
  });

  it('encodes borderColor', () => {
    const out: number[] = [];
    encodeStylePatch({ borderColor: '#0000ff' }, out);
    expect(out[0]).toBe(TAG.BORDER_COLOR);
    expect(out).toHaveLength(5);
    expect(out[1]).toBeCloseTo(0, 5); // r
    expect(out[2]).toBeCloseTo(0, 5); // g
    expect(out[3]).toBeCloseTo(1, 5); // b
    expect(out[4]).toBeCloseTo(1, 5); // a
  });

  it('encodes transparent color', () => {
    const out: number[] = [];
    encodeStylePatch({ backgroundColor: 'transparent' }, out);
    expect(out[0]).toBe(TAG.BACKGROUND_COLOR);
    expect(out[1]).toBe(0);
    expect(out[2]).toBe(0);
    expect(out[3]).toBe(0);
    expect(out[4]).toBe(0);
  });
});

describe('encodeStylePatch – dimension properties', () => {
  it('encodes width in px', () => {
    const out: number[] = [];
    encodeStylePatch({ width: '100px' }, out);
    expect(out[0]).toBe(TAG.WIDTH);
    expect(out[1]).toBe(100);
    expect(out[2]).toBe(0); // UNIT_CODE.px = 0
  });

  it('encodes width in percent', () => {
    const out: number[] = [];
    encodeStylePatch({ width: '50%' }, out);
    expect(out[0]).toBe(TAG.WIDTH);
    expect(out[1]).toBe(50);
    expect(out[2]).toBe(1); // UNIT_CODE.percent = 1
  });

  it('encodes width as auto', () => {
    const out: number[] = [];
    encodeStylePatch({ width: 'auto' }, out);
    expect(out[0]).toBe(TAG.WIDTH);
    expect(out[1]).toBe(0);
    expect(out[2]).toBe(2); // UNIT_CODE.auto = 2
  });

  it('encodes height', () => {
    const out: number[] = [];
    encodeStylePatch({ height: 200 }, out);
    expect(out[0]).toBe(TAG.HEIGHT);
    expect(out[1]).toBe(200);
    expect(out[2]).toBe(0); // px
  });

  it('encodes minWidth', () => {
    const out: number[] = [];
    encodeStylePatch({ minWidth: '10px' }, out);
    expect(out[0]).toBe(TAG.MIN_WIDTH);
    expect(out[1]).toBe(10);
    expect(out[2]).toBe(0);
  });

  it('encodes minHeight', () => {
    const out: number[] = [];
    encodeStylePatch({ minHeight: 20 }, out);
    expect(out[0]).toBe(TAG.MIN_HEIGHT);
    expect(out[1]).toBe(20);
    expect(out[2]).toBe(0);
  });

  it('encodes maxWidth', () => {
    const out: number[] = [];
    encodeStylePatch({ maxWidth: '100%' }, out);
    expect(out[0]).toBe(TAG.MAX_WIDTH);
    expect(out[1]).toBe(100);
    expect(out[2]).toBe(1);
  });

  it('encodes maxHeight', () => {
    const out: number[] = [];
    encodeStylePatch({ maxHeight: 'auto' }, out);
    expect(out[0]).toBe(TAG.MAX_HEIGHT);
    expect(out[1]).toBe(0);
    expect(out[2]).toBe(2);
  });
});

describe('encodeStylePatch – enum properties', () => {
  it('encodes display: flex', () => {
    const out: number[] = [];
    encodeStylePatch({ display: 'flex' }, out);
    expect(out[0]).toBe(TAG.DISPLAY);
    expect(out[1]).toBe(0); // DISPLAY.flex = 0
  });

  it('encodes display: grid', () => {
    const out: number[] = [];
    encodeStylePatch({ display: 'grid' }, out);
    expect(out[0]).toBe(TAG.DISPLAY);
    expect(out[1]).toBe(1);
  });

  it('encodes display: block', () => {
    const out: number[] = [];
    encodeStylePatch({ display: 'block' }, out);
    expect(out[0]).toBe(TAG.DISPLAY);
    expect(out[1]).toBe(2);
  });

  it('encodes display: none', () => {
    const out: number[] = [];
    encodeStylePatch({ display: 'none' }, out);
    expect(out[0]).toBe(TAG.DISPLAY);
    expect(out[1]).toBe(3);
  });

  it('encodes flexDirection: row', () => {
    const out: number[] = [];
    encodeStylePatch({ flexDirection: 'row' }, out);
    expect(out[0]).toBe(TAG.FLEX_DIRECTION);
    expect(out[1]).toBe(0);
  });

  it('encodes flexDirection: column', () => {
    const out: number[] = [];
    encodeStylePatch({ flexDirection: 'column' }, out);
    expect(out[0]).toBe(TAG.FLEX_DIRECTION);
    expect(out[1]).toBe(1);
  });

  it('encodes flexDirection: row-reverse', () => {
    const out: number[] = [];
    encodeStylePatch({ flexDirection: 'row-reverse' }, out);
    expect(out[0]).toBe(TAG.FLEX_DIRECTION);
    expect(out[1]).toBe(2);
  });

  it('encodes flexDirection: column-reverse', () => {
    const out: number[] = [];
    encodeStylePatch({ flexDirection: 'column-reverse' }, out);
    expect(out[0]).toBe(TAG.FLEX_DIRECTION);
    expect(out[1]).toBe(3);
  });

  it('encodes alignItems: flex-start', () => {
    const out: number[] = [];
    encodeStylePatch({ alignItems: 'flex-start' }, out);
    expect(out[0]).toBe(TAG.ALIGN_ITEMS);
    expect(out[1]).toBe(0);
  });

  it('encodes alignItems: center', () => {
    const out: number[] = [];
    encodeStylePatch({ alignItems: 'center' }, out);
    expect(out[0]).toBe(TAG.ALIGN_ITEMS);
    expect(out[1]).toBe(2);
  });

  it('encodes justifyContent: flex-start', () => {
    const out: number[] = [];
    encodeStylePatch({ justifyContent: 'flex-start' }, out);
    expect(out[0]).toBe(TAG.JUSTIFY_CONTENT);
    expect(out[1]).toBe(0);
  });

  it('encodes justifyContent: space-between', () => {
    const out: number[] = [];
    encodeStylePatch({ justifyContent: 'space-between' }, out);
    expect(out[0]).toBe(TAG.JUSTIFY_CONTENT);
    expect(out[1]).toBe(3);
  });

  it('encodes justifyContent: space-evenly', () => {
    const out: number[] = [];
    encodeStylePatch({ justifyContent: 'space-evenly' }, out);
    expect(out[0]).toBe(TAG.JUSTIFY_CONTENT);
    expect(out[1]).toBe(5);
  });
});

describe('encodeStylePatch – fontFamily', () => {
  it('encodes fontFamily as [TAG.FONT_FAMILY, byteLength, ...utf8bytes]', () => {
    const out: number[] = [];
    encodeStylePatch({ fontFamily: 'Inter' }, out);
    const bytes = new TextEncoder().encode('Inter');
    expect(out[0]).toBe(TAG.FONT_FAMILY);
    expect(out[1]).toBe(bytes.length);
    for (let i = 0; i < bytes.length; i++) {
      expect(out[2 + i]).toBe(bytes[i]);
    }
  });

  it('encodes multi-byte fontFamily name', () => {
    const out: number[] = [];
    encodeStylePatch({ fontFamily: 'Noto Sans JP' }, out);
    const bytes = new TextEncoder().encode('Noto Sans JP');
    expect(out[0]).toBe(TAG.FONT_FAMILY);
    expect(out[1]).toBe(bytes.length);
    expect(out).toHaveLength(2 + bytes.length);
  });
});

describe('encodeStylePatch – numeric properties', () => {
  it('encodes fontWeight', () => {
    const out: number[] = [];
    encodeStylePatch({ fontWeight: 700 }, out);
    expect(out[0]).toBe(TAG.FONT_WEIGHT);
    expect(out[1]).toBe(700);
  });

  it('encodes fontSize', () => {
    const out: number[] = [];
    encodeStylePatch({ fontSize: 16 }, out);
    expect(out[0]).toBe(TAG.FONT_SIZE);
    expect(out[1]).toBe(16);
  });

  it('encodes opacity', () => {
    const out: number[] = [];
    encodeStylePatch({ opacity: 0.5 }, out);
    expect(out[0]).toBe(TAG.OPACITY);
    expect(out[1]).toBe(0.5);
  });

  it('encodes borderRadius', () => {
    const out: number[] = [];
    encodeStylePatch({ borderRadius: 8 }, out);
    expect(out[0]).toBe(TAG.BORDER_RADIUS);
    expect(out[1]).toBe(8);
  });

  it('encodes zIndex', () => {
    const out: number[] = [];
    encodeStylePatch({ zIndex: 10 }, out);
    expect(out[0]).toBe(TAG.Z_INDEX);
    expect(out[1]).toBe(10);
  });

  it('encodes flexGrow', () => {
    const out: number[] = [];
    encodeStylePatch({ flexGrow: 1 }, out);
    expect(out[0]).toBe(TAG.FLEX_GROW);
    expect(out[1]).toBe(1);
  });

  it('encodes borderWidth', () => {
    const out: number[] = [];
    encodeStylePatch({ borderWidth: 2 }, out);
    expect(out[0]).toBe(TAG.BORDER_WIDTH);
    expect(out[1]).toBe(2);
  });
});

describe('encodeStylePatch – padding and margin', () => {
  it('encodes padding', () => {
    const out: number[] = [];
    encodeStylePatch({ padding: '8px' }, out);
    expect(out[0]).toBe(TAG.PADDING);
    expect(out[1]).toBe(8);
    expect(out[2]).toBe(0);
  });

  it('encodes paddingTop/Right/Bottom/Left', () => {
    const out: number[] = [];
    encodeStylePatch({ paddingTop: 1, paddingRight: 2, paddingBottom: 3, paddingLeft: 4 }, out);
    expect(out[0]).toBe(TAG.PADDING_TOP);
    expect(out[1]).toBe(1);
    expect(out[3]).toBe(TAG.PADDING_RIGHT);
    expect(out[4]).toBe(2);
    expect(out[6]).toBe(TAG.PADDING_BOTTOM);
    expect(out[7]).toBe(3);
    expect(out[9]).toBe(TAG.PADDING_LEFT);
    expect(out[10]).toBe(4);
  });

  it('encodes margin', () => {
    const out: number[] = [];
    encodeStylePatch({ margin: 'auto' }, out);
    expect(out[0]).toBe(TAG.MARGIN);
    expect(out[2]).toBe(2); // auto unit
  });
});

describe('encodeStylePatch – gap', () => {
  it('encodes gap', () => {
    const out: number[] = [];
    encodeStylePatch({ gap: '16px' }, out);
    expect(out[0]).toBe(TAG.GAP);
    expect(out[1]).toBe(16);
    expect(out[2]).toBe(0);
  });
});

describe('encodeStylePatch – null/undefined skipping', () => {
  it('skips null values', () => {
    const out: number[] = [];
    // color: null is inheritable, fontSize: null is inheritable — both skipped in encode
    encodeStylePatch({ color: null, fontSize: null } as any, out);
    expect(out).toHaveLength(0);
  });

  it('skips undefined values', () => {
    const out: number[] = [];
    encodeStylePatch({ color: undefined } as any, out);
    expect(out).toHaveLength(0);
  });

  it('appends to existing out array without clobbering', () => {
    const out: number[] = [42];
    encodeStylePatch({ opacity: 1 }, out);
    expect(out[0]).toBe(42);
    expect(out[1]).toBe(TAG.OPACITY);
    expect(out[2]).toBe(1);
  });
});

// ── unsetKindsOf ──────────────────────────────────────────────────────────────

describe('unsetKindsOf', () => {
  it('color null → UNSET_KIND.color', () => {
    const kinds = unsetKindsOf({ color: null } as any);
    expect(kinds).toContain(UNSET_KIND.color);
  });

  it('fontSize null → UNSET_KIND.fontSize', () => {
    const kinds = unsetKindsOf({ fontSize: null } as any);
    expect(kinds).toContain(UNSET_KIND.fontSize);
  });

  it('fontFamily null → UNSET_KIND.fontFamily', () => {
    const kinds = unsetKindsOf({ fontFamily: null } as any);
    expect(kinds).toContain(UNSET_KIND.fontFamily);
  });

  it('fontWeight null → UNSET_KIND.fontWeight', () => {
    const kinds = unsetKindsOf({ fontWeight: null } as any);
    expect(kinds).toContain(UNSET_KIND.fontWeight);
  });

  it('multiple inheritable nulls returns all codes', () => {
    const kinds = unsetKindsOf({ color: null, fontSize: null, fontFamily: null } as any);
    expect(kinds).toHaveLength(3);
    expect(kinds).toContain(UNSET_KIND.color);
    expect(kinds).toContain(UNSET_KIND.fontSize);
    expect(kinds).toContain(UNSET_KIND.fontFamily);
  });

  it('non-null values are skipped', () => {
    const kinds = unsetKindsOf({ color: null, fontSize: 16 } as any);
    expect(kinds).toHaveLength(1);
    expect(kinds).toContain(UNSET_KIND.color);
  });

  it('non-inheritable null throws', () => {
    expect(() => unsetKindsOf({ width: null } as any)).toThrow();
  });

  it('non-inheritable null (opacity) throws', () => {
    expect(() => unsetKindsOf({ opacity: null } as any)).toThrow();
  });
});
