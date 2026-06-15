import { describe, expect, it } from 'vitest';
import { elementPropertyModel, renderElementProperty } from './gen-element-property.mjs';

/** A miniature spec section: order and fields mirror element_properties.json. */
const SAMPLE = [
  { name: 'value', opKind: 'text-content', coerce: 'stringify-nullable' },
  { name: 'placeholder', opKind: 'placeholder', coerce: 'string-or-empty' },
  { name: 'disabled', opKind: 'disabled', coerce: 'boolean' },
];

describe('elementPropertyModel', () => {
  it('derives the property vocabulary from spec order (single source)', () => {
    const model = elementPropertyModel({ element_properties: SAMPLE });
    expect(model.names).toEqual(['value', 'placeholder', 'disabled']);
  });

  it('shapes each op payload from its coerce strategy (text → text:string, boolean → <kind>:boolean)', () => {
    const model = elementPropertyModel({ element_properties: SAMPLE });
    expect(model.ops).toEqual([
      { kind: 'text-content', field: 'text', tsType: 'string' },
      { kind: 'placeholder', field: 'text', tsType: 'string' },
      { kind: 'disabled', field: 'disabled', tsType: 'boolean' },
    ]);
  });

  it('writes the value-coercion expression exactly once per property (the single seam)', () => {
    const model = elementPropertyModel({ element_properties: SAMPLE });
    expect(model.cases).toEqual([
      { name: 'value', kind: 'text-content', field: 'text', expr: "value == null ? '' : String(value)" },
      { name: 'placeholder', kind: 'placeholder', field: 'text', expr: "typeof value === 'string' ? value : ''" },
      { name: 'disabled', kind: 'disabled', field: 'disabled', expr: 'Boolean(value)' },
    ]);
  });

  it('rejects an unknown coerce strategy at generation time', () => {
    expect(() =>
      elementPropertyModel({ element_properties: [{ name: 'x', opKind: 'x', coerce: 'bogus' }] }),
    ).toThrow(/unknown coerce strategy/);
  });

  it('renders one op-union member and one coerce case per spec property', () => {
    const out = renderElementProperty(elementPropertyModel({ element_properties: SAMPLE }));
    // Every spec property reaches the closed vocabulary, the op union, and coerce.
    expect(out).toContain(`export const ELEMENT_PROPERTY_NAMES = ["value","placeholder","disabled"] as const;`);
    expect(out).toContain(`| { kind: 'text-content'; text: string }`);
    expect(out).toContain(`| { kind: 'disabled'; disabled: boolean }`);
    expect(out).toContain(`case 'value':`);
    expect(out).toContain(`return { kind: 'text-content', text: value == null ? '' : String(value) };`);
    // The op-kind match lives in exactly one place: the shared dispatch.
    expect(out).toContain('export type ElementPropertyEffects<R>');
    expect(out).toContain('export function dispatchElementPropertyOp<R>');
  });
});
