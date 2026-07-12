import type { ElementKind, StylePatch } from '@torimi/tsubame-renderer-protocol';
import { CATALOG_BY_KEY, formatDomCSSValue } from '@torimi/tsubame-hayate-css-catalog';
import type { CatalogEntry } from '@torimi/tsubame-hayate-css-catalog';

export interface StylePatchDeclaration {
  /** CSSOM `style` オブジェクト用の camelCase キー */
  readonly cssName: string;
  /** CSS ルールテキスト用の kebab-case プロパティ */
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
 * パッチ → 順序付き CSS 宣言列（カタログの DOM-extras を含む）。
 *
 * Style Channel ゲートはここでは適用しない。各レンダラーの手前の境界で一度
 * 実行される（ADR-0008、`withTextLocalGate`）ため、ここに届くパッチは
 * フィルタ済み。`kind` は呼び出し側と kind 別カタログ挙動のためシグネチャに
 * 残している。
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
 * 宣言列を CSS ルール本体（`property:value;...`）へ連結する。
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
