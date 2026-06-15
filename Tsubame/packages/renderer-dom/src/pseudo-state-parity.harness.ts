import type { ElementKind, StylePatch } from '@tsubame/renderer-protocol';
import { PSEUDO_STYLE_KEYS_BY_PRIORITY, gateTextLocalPatch } from '@tsubame/renderer-protocol';
import type { PseudoStyleKey } from '@tsubame/renderer-protocol';
import {
  declarationsFromStylePatch,
  type StylePatchDeclaration,
} from './style-declarations.js';

export interface ParityInteraction {
  readonly focus?: boolean;
  readonly hover?: boolean;
  readonly active?: boolean;
}

export interface ParityExpectedProperty {
  readonly property: string;
  readonly value: string;
  readonly domOnly?: boolean;
}

export interface PseudoStateParityFixture {
  readonly name: string;
  readonly elementKind: 'view' | 'text';
  readonly pseudo: Partial<Record<PseudoStyleKey, StylePatch>>;
  readonly interaction: ParityInteraction;
  readonly expected: {
    readonly properties: readonly ParityExpectedProperty[];
  };
}

function interactionActive(key: PseudoStyleKey, interaction: ParityInteraction): boolean {
  switch (key) {
    case ':focus':
      return interaction.focus === true;
    case ':hover':
      return interaction.hover === true;
    case ':active':
      return interaction.active === true;
    default: {
      const _exhaustive: never = key;
      return _exhaustive;
    }
  }
}

/**
 * Merge active pseudo patches in priority order, then run the declaration
 * emitter. The Style Channel gate is applied first — in production it runs in
 * the seam before the DOM renderer (Tsubame ADR-0008), so this harness gates
 * here to model the same post-seam pipeline the corpus describes.
 */
export function resolvePseudoDeclarations(
  kind: ElementKind,
  pseudo: Partial<Record<PseudoStyleKey, StylePatch>>,
  interaction: ParityInteraction,
): StylePatchDeclaration[] {
  const byProperty = new Map<string, StylePatchDeclaration>();

  for (const key of PSEUDO_STYLE_KEYS_BY_PRIORITY) {
    if (!interactionActive(key, interaction)) continue;
    const patch = pseudo[key];
    if (patch === undefined) continue;

    const gated = gateTextLocalPatch(kind, patch);
    for (const decl of declarationsFromStylePatch(kind, gated, { onUnknownKey: 'skip' })) {
      byProperty.set(decl.cssProperty, decl);
    }
  }

  return [...byProperty.values()];
}

export function declarationsToPropertyMap(
  declarations: readonly StylePatchDeclaration[],
): Map<string, string> {
  return new Map(declarations.map((d) => [d.cssProperty, d.value]));
}

export function expectedPropertyMap(
  fixture: PseudoStateParityFixture,
  side: 'ts' | 'rust',
): Map<string, string> {
  const map = new Map<string, string>();
  for (const prop of fixture.expected.properties) {
    if (side === 'rust' && prop.domOnly === true) continue;
    map.set(prop.property, prop.value);
  }
  return map;
}
