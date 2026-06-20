import type { ElementKind, StylePatch } from '@tsubame/renderer-protocol';
import { CATALOG_BY_KEY, formatDomCSSValue } from '@tsubame/hayate-css-catalog';
import type { CatalogEntry } from '@tsubame/hayate-css-catalog';

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

/**
 * Patch → ordered CSS declarations, including catalog DOM-extras.
 *
 * The Style Channel gate is *not* applied here — it runs once in the seam before
 * any renderer (Tsubame ADR-0008, `withTextLocalGate`), so the patch reaching
 * this emitter is already filtered. `kind` is kept on the signature for callers
 * and potential kind-specific catalog behavior.
 */
export function declarationsFromStylePatch(
  kind: ElementKind,
  patch: StylePatch,
  options: { onUnknownKey: UnknownStyleKeyPolicy },
): StylePatchDeclaration[] {
  const declarations: StylePatchDeclaration[] = [];

  for (const key in patch) {
    const k = key as keyof StylePatch;
    const value = patch[k];
    if (value === undefined) continue;

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

/**
 * Join declarations into a CSS rule body (`property:value;...`).
 *
 * `important` を立てると各宣言へ `!important` を付与する。ベーススタイルは
 * インライン（`el.style`）に載るため、@media variant がベースを上書きするには
 * `!important` が必須（インライン宣言は通常のセレクタ規則より優先される）。
 */
export function declarationsToRuleBody(
  declarations: readonly StylePatchDeclaration[],
  options: { important?: boolean } = {},
): string {
  const suffix = options.important === true ? ' !important' : '';
  return declarations
    .filter((decl) => decl.value !== '')
    .map((decl) => `${decl.cssProperty}:${decl.value}${suffix}`)
    .join(';');
}
