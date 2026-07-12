import { describe, expect, it } from 'vitest';
import { loadProtocolSpec } from '@torimi/hayate-protocol-spec/load';
import { classify, tsType, wireKind } from './value-type.mjs';

function sampleTag(encodeFrom, params, variableLength = false) {
  return {
    name: 'SAMPLE',
    encodeFrom,
    variable_length: variableLength,
    params: params.map(([name, type]) => ({ name, type })),
  };
}

describe('classify', () => {
  it('derives color from css-color encodeFrom', () => {
    const tag = sampleTag('css-color', [['c', 'color']]);
    expect(classify(tag)).toEqual({ type: 'color' });
  });

  it('derives dimension from encodeFrom', () => {
    const tag = sampleTag('dimension', [['d', 'dimension']]);
    expect(classify(tag)).toEqual({ type: 'dimension' });
  });

  it('derives scalar from f32 encodeFrom', () => {
    const tag = sampleTag('f32', [['value', 'f32']]);
    expect(classify(tag)).toEqual({ type: 'scalar' });
  });

  it('derives enum from encodeFrom', () => {
    const tag = sampleTag('enum:display', [['value', 'display']]);
    expect(classify(tag)).toEqual({ type: 'enum', kind: 'display' });
  });

  it('derives font family from encodeFrom and param type', () => {
    const tag = sampleTag('font-family', [['family', 'string']], true);
    expect(classify(tag)).toEqual({ type: 'fontFamily' });
  });

  it('derives dimension list from encodeFrom', () => {
    const tag = sampleTag('dimension-list', [['tracks', 'dimension']], true);
    expect(classify(tag)).toEqual({ type: 'dimensionList' });
  });

  it('derives z-index from encodeFrom and i32 param', () => {
    const tag = sampleTag('z-index', [['value', 'i32']]);
    expect(classify(tag)).toEqual({ type: 'zIndex' });
  });

  it('derives shadow list from encodeFrom', () => {
    const tag = sampleTag('shadow-list', [['shadows', 'shadow']], true);
    expect(classify(tag)).toEqual({ type: 'shadowList' });
  });

  it('derives grid placement from encodeFrom and grid_placement param', () => {
    const tag = sampleTag('grid-placement', [['placement', 'grid_placement']]);
    expect(classify(tag)).toEqual({ type: 'gridPlacement' });
  });

  it('requires a grid_placement param for grid-placement', () => {
    const tag = sampleTag('grid-placement', [['placement', 'u32']]);
    expect(() => classify(tag)).toThrow(/grid_placement/);
  });

  it('requires variable_length for shadow-list', () => {
    const tag = sampleTag('shadow-list', [['shadows', 'shadow']], false);
    expect(() => classify(tag)).toThrow(/variable_length/);
  });

  it('classifies every style tag in the protocol spec', () => {
    const proto = loadProtocolSpec();
    expect(proto.style_tags.length).toBeGreaterThan(0);
    for (const tag of proto.style_tags) {
      expect(() => classify(tag)).not.toThrow();
    }
  });
});

describe('wireKind', () => {
  it('maps closed value types to catalog wire kinds', () => {
    expect(wireKind({ type: 'color' })).toBe('color');
    expect(wireKind({ type: 'dimension' })).toBe('dimension');
    expect(wireKind({ type: 'scalar' })).toBe('f32');
    expect(wireKind({ type: 'dimensionList' })).toBe('dimensionList');
    expect(wireKind({ type: 'shadowList' })).toBe('shadowList');
    expect(wireKind({ type: 'fontFamily' })).toBe('fontFamily');
    expect(wireKind({ type: 'zIndex' })).toBe('zIndex');
    expect(wireKind({ type: 'gridPlacement' })).toBe('gridPlacement');
    expect(wireKind({ type: 'enum', kind: 'display' })).toBe('display');
    expect(wireKind({ type: 'enum', kind: 'flex_direction' })).toBe('flexDirection');
  });

  it('derives fontFamily and zIndex wire kinds from encodeFrom, not tag names', () => {
    const proto = loadProtocolSpec();
    const fontFamily = proto.style_tags.find((tag) => tag.name === 'FONT_FAMILY');
    const zIndex = proto.style_tags.find((tag) => tag.name === 'Z_INDEX');
    expect(wireKind(classify(fontFamily))).toBe('fontFamily');
    expect(wireKind(classify(zIndex))).toBe('zIndex');
  });
});

describe('tsType', () => {
  it('maps closed value types to TypeScript types', () => {
    expect(tsType({ type: 'color' })).toBe('string');
    expect(tsType({ type: 'dimension' })).toBe('HayateDimension');
    expect(tsType({ type: 'scalar' })).toBe('number');
    expect(tsType({ type: 'dimensionList' })).toBe('HayateDimension[]');
    expect(tsType({ type: 'shadowList' })).toBe('HayateShadow[]');
    expect(tsType({ type: 'fontFamily' })).toBe('string');
    expect(tsType({ type: 'zIndex' })).toBe('number');
    expect(tsType({ type: 'gridPlacement' })).toBe('HayateGridPlacement');
    expect(tsType({ type: 'enum', kind: 'display' })).toBe('Display');
    expect(tsType({ type: 'enum', kind: 'flex_direction' })).toBe('FlexDirection');
  });
});

