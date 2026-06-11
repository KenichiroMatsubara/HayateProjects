import type { StylePatch } from '@tsubame/renderer-protocol';
import { CATALOG_BY_KEY, formatDomCSSValue } from '@tsubame/hayate-css-catalog';
import type { CatalogEntry } from '@tsubame/hayate-css-catalog';
import { shouldApplyTextLocalPatch } from './text-style-semantics.js';

export interface StylePatchDeclaration {
  /** camelCase key for CSSOM `style` object */
  readonly cssName: string;
  /** kebab-case property for CSS rule text */
  readonly cssProperty: string;
  readonly value: string;
}

export type UnknownStyleKeyPolicy = 'throw' | 'skip';

function extrasFromEntry(entry: CatalogEntry, value: unknown): StylePatchDeclaration[] {
  if (!entry.domExtras) return [];
  const n = typeof value === 'number' ? value : Number(value);
  return entry.domExtras.map((extra) => ({
    cssName: extra.cssName,
    cssProperty: extra.cssProperty,
    value: n > 0 ? extra.whenPositive : extra.whenZero,
  }));
}

/** Patch → ordered CSS declarations, including catalog DOM-extras and text-channel gating. */
export function declarationsFromStylePatch(
  el: HTMLElement,
  patch: StylePatch,
  options: { onUnknownKey: UnknownStyleKeyPolicy },
): StylePatchDeclaration[] {
  const declarations: StylePatchDeclaration[] = [];

  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;
    if (!shouldApplyTextLocalPatch(el, k as string)) continue;

    const entry = CATALOG_BY_KEY[k as string];
    if (entry === undefined) {
      if (options.onUnknownKey === 'throw') {
        throw new Error(`DOMRenderer: unknown Hayate style property "${k}"`);
      }
      continue;
    }

    if (value === null) {
      declarations.push({
        cssName: entry.cssName,
        cssProperty: entry.cssProperty,
        value: '',
      });
      continue;
    }

    declarations.push({
      cssName: entry.cssName,
      cssProperty: entry.cssProperty,
      value: formatDomCSSValue(entry, value),
    });
    declarations.push(...extrasFromEntry(entry, value));
  }

  return declarations;
}

/** Join declarations into a CSS rule body (`property:value;...`). */
export function declarationsToRuleBody(declarations: readonly StylePatchDeclaration[]): string {
  return declarations
    .filter((decl) => decl.value !== '')
    .map((decl) => `${decl.cssProperty}:${decl.value}`)
    .join(';');
}
