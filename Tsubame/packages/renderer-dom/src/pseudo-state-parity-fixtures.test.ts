import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';
import {
  declarationsToPropertyMap,
  expectedPropertyMap,
  resolvePseudoDeclarations,
  type PseudoStateParityFixture,
} from './pseudo-state-parity.harness.js';

const fixturesPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../Hayate/proto/spec/fixtures/pseudo_state_parity.json',
);

const fixtures = JSON.parse(readFileSync(fixturesPath, 'utf8')) as PseudoStateParityFixture[];

describe('pseudo-state parity corpus (DOM declaration emitter)', () => {
  for (const fixture of fixtures) {
    it(fixture.name, () => {
      const declarations = resolvePseudoDeclarations(
        fixture.elementKind,
        fixture.pseudo,
        fixture.interaction,
      );
      const actual = declarationsToPropertyMap(declarations);
      const expected = expectedPropertyMap(fixture, 'ts');

      expect(actual.size).toBe(expected.size);
      for (const [property, value] of expected) {
        expect(actual.get(property), `${fixture.name}: ${property}`).toBe(value);
      }
    });
  }

  it('corpus catches dropped border-style', () => {
    const fixture = fixtures.find((f) => f.name === 'hover_border_style_dashed')!;
    const declarations = resolvePseudoDeclarations(
      fixture.elementKind,
      fixture.pseudo,
      fixture.interaction,
    );
    const withoutBorderStyle = declarations.filter((d) => d.cssProperty !== 'border-style');
    const actual = declarationsToPropertyMap(withoutBorderStyle);
    const expected = expectedPropertyMap(fixture, 'ts');

    expect(actual.get('border-style')).toBeUndefined();
    expect(actual).not.toEqual(expected);
  });

  it('corpus catches flipped pseudo priority', () => {
    const fixture = fixtures.find((f) => f.name === 'hover_active_priority_active_wins')!;
    // Wrong order: hover after active (focus < active < hover)
    const byProperty = new Map<string, string>();
    for (const key of [':active', ':hover'] as const) {
      const patch = fixture.pseudo[key];
      if (patch === undefined) continue;
      for (const decl of resolvePseudoDeclarations(fixture.elementKind, { [key]: patch }, {
        [key.slice(1)]: true,
      })) {
        byProperty.set(decl.cssProperty, decl.value);
      }
    }
    // Simulate hover winning by applying hover last in wrong band order
    const hoverDecls = resolvePseudoDeclarations(
      fixture.elementKind,
      { ':hover': fixture.pseudo[':hover']! },
      { hover: true },
    );
    for (const decl of hoverDecls) {
      byProperty.set(decl.cssProperty, decl.value);
    }

    expect(byProperty.get('background-color')).toBe('#00ff00');
    expect(byProperty.get('background-color')).not.toBe(
      expectedPropertyMap(fixture, 'ts').get('background-color'),
    );
  });
});
