import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import {
  loadProtocolSpec,
  tagToPatchKey,
  toCamelCase,
} from '@hayate/protocol-spec/load';
import { classify, wireKind } from './value-type.mjs';

const outDir = join(dirname(fileURLToPath(import.meta.url)), '../generated');
const jsonOutPath = join(outDir, 'catalog.json');
const tsOutPath = join(outDir, 'catalog.ts');

function propertyToCamelCase(kebab) {
  return kebab.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
}

function domFormatFromSpec(format) {
  if (format.startsWith('enum:')) return 'enum';
  return format;
}

// enum 値のうち、DOM CSS 形がパッチのキーワード形（enumTsKey: snake→kebab）と
// 異なるものだけを上書きする。Rust 側 value_type.rs::enum_css_collect と対の関係。
const ENUM_CSS_OVERRIDES = {
  // grid-auto-flow の dense は CSS では空白区切り（`row dense` / `column dense`）。
  grid_auto_flow: { 'row-dense': 'row dense', 'column-dense': 'column dense' },
};

function enumCssFromSpec(format) {
  if (!format.startsWith('enum:')) return undefined;
  return ENUM_CSS_OVERRIDES[format.slice('enum:'.length)];
}

function domExtrasFromSpec(extras) {
  if (!extras?.length) return undefined;
  return extras.map((extra) => ({
    cssName: propertyToCamelCase(extra.property),
    cssProperty: extra.property,
    whenPositive: extra.whenPositive,
    whenZero: extra.whenZero,
  }));
}

export function generateCatalog() {
  const proto = loadProtocolSpec();

  const unsetByPatchKey = new Map();
  for (const uk of proto.unset_kinds ?? []) {
    unsetByPatchKey.set(tagToPatchKey(uk.name), {
      name: toCamelCase(uk.name),
      value: uk.value,
    });
  }

  const catalog = (proto.style_tags ?? []).map((tag) => {
    const patchKey = tagToPatchKey(tag.name);
    const kind = wireKind(classify(tag));
    const unset = unsetByPatchKey.get(patchKey);
    const domCss = tag.domCss;
    if (domCss == null) {
      throw new Error(`style_tags.${tag.name}: missing domCss`);
    }
    const entry = {
      patchKey,
      tag: tag.value,
      unsetKind: unset ? unset.value : null,
      wireKind: kind,
      domFormat: domFormatFromSpec(domCss.format),
      cssName: propertyToCamelCase(domCss.property),
      cssProperty: domCss.property,
      targets: ['packet', 'css'],
    };
    const extras = domExtrasFromSpec(domCss.extras);
    if (extras) entry.domExtras = extras;
    const enumCss = enumCssFromSpec(domCss.format);
    if (enumCss) entry.enumCss = enumCss;
    return entry;
  });

  mkdirSync(outDir, { recursive: true });
  writeFileSync(jsonOutPath, `${JSON.stringify(catalog, null, 2)}\n`, 'utf8');

  const tsLines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec',
    '',
    "import type { HayateDimension, HayateShadow } from '@tsubame/renderer-protocol';",
    '',
    "export type WireKind = 'color' | 'dimension' | 'dimensionList' | 'shadowList' | 'display' | 'flexDirection' | 'flexWrap' | 'alignItems' | 'alignSelf' | 'alignContent' | 'justifyContent' | 'fontStyle' | 'textDecoration' | 'borderStyle' | 'cursor' | 'overflow' | 'textOverflow' | 'position' | 'transitionTiming' | 'boxSizing' | 'gridAutoFlow' | 'justifyItems' | 'justifySelf' | 'gridPlacement' | 'f32' | 'u32' | 'zIndex' | 'fontFamily';",
    "export type DomFormat = 'dimension' | 'dimension-list' | 'shadow-list' | 'px' | 'ms' | 'number' | 'integer' | 'color' | 'enum' | 'string' | 'grid-placement';",
    '',
    'export interface DomExtra {',
    '  readonly cssName: string;',
    '  readonly cssProperty: string;',
    '  readonly whenPositive: string;',
    '  readonly whenZero: string;',
    '}',
    '',
    'export interface CatalogEntry {',
    '  readonly patchKey: string;',
    '  readonly tag: number;',
    '  readonly unsetKind: number | null;',
    '  readonly wireKind: WireKind;',
    '  readonly domFormat: DomFormat;',
    '  readonly cssName: string;',
    '  readonly cssProperty: string;',
    '  readonly targets: readonly ("packet" | "css")[];',
    '  readonly domExtras?: readonly DomExtra[];',
    '  // enum 値のうち、DOM CSS 形がパッチのキーワード形と異なるものの上書き表',
    '  // （例: grid-auto-flow の `row-dense` → CSS `row dense`）。',
    '  readonly enumCss?: Readonly<Record<string, string>>;',
    '}',
    '',
    `export const HAYATE_CSS_CATALOG: readonly CatalogEntry[] = ${JSON.stringify(catalog, null, 2)};`,
    '',
    'export const CATALOG_BY_KEY: Readonly<Record<string, CatalogEntry>> = Object.fromEntries(',
    '  HAYATE_CSS_CATALOG.map((e) => [e.patchKey, e]),',
    ');',
    '',
    'export const CATALOG_BY_TAG: Readonly<Record<number, CatalogEntry>> = Object.fromEntries(',
    '  HAYATE_CSS_CATALOG.map((e) => [e.tag, e]),',
    ');',
    '',
    'export interface StyleEncodeEntry {',
    '  readonly key: string;',
    '  readonly tag: number;',
    '  readonly kind: WireKind;',
    '}',
    '',
    'export const STYLE_ENCODE_META: readonly StyleEncodeEntry[] = HAYATE_CSS_CATALOG.filter(',
    '  (e) => e.targets.includes("packet"),',
    ').map((e) => ({ key: e.patchKey, tag: e.tag, kind: e.wireKind }));',
    '',
    'export const INHERITED_UNSET: Partial<Record<string, number>> = Object.fromEntries(',
    '  HAYATE_CSS_CATALOG.filter((e): e is CatalogEntry & { unsetKind: number } => e.unsetKind !== null)',
    '    .map((e) => [e.patchKey, e.unsetKind]),',
    ');',
    '',
    'function formatDimension(value: HayateDimension): string {',
    '  return typeof value === "number" ? `${value}px` : value;',
    '}',
    '',
    'function formatDimensionList(value: unknown): string {',
    '  if (!Array.isArray(value)) {',
    '    throw new Error("DOMRenderer: grid track list must be an array");',
    '  }',
    '  return value.map((item) => formatDimension(item as HayateDimension)).join(" ");',
    '}',
    '',
    'function formatShadow(shadow: HayateShadow): string {',
    '  const parts = [];',
    '  if (shadow.inset) parts.push("inset");',
    '  parts.push(`${shadow.offsetX}px`, `${shadow.offsetY}px`, `${shadow.blur}px`, `${shadow.spread}px`, shadow.color);',
    '  return parts.join(" ");',
    '}',
    '',
    'function formatShadowList(value: unknown): string {',
    '  if (!Array.isArray(value)) {',
    '    throw new Error("DOMRenderer: box-shadow must be an array");',
    '  }',
    '  if (value.length === 0) return "none";',
    '  return value.map((item) => formatShadow(item as HayateShadow)).join(", ");',
    '}',
    '',
    'function formatGridLine(line: unknown): string {',
    '  if (line === undefined || line === null || line === "auto") return "auto";',
    '  if (typeof line === "number") return String(line);',
    '  if (typeof line === "object" && "span" in (line as Record<string, unknown>)) {',
    '    return `span ${(line as { span: number }).span}`;',
    '  }',
    '  throw new Error("DOMRenderer: unsupported grid placement");',
    '}',
    '',
    'function formatGridPlacement(value: unknown): string {',
    '  const p = (value ?? {}) as { start?: unknown; end?: unknown };',
    '  return `${formatGridLine(p.start)} / ${formatGridLine(p.end)}`;',
    '}',
    '',
    'export function formatDomCSSValue(entry: CatalogEntry, value: unknown): string {',
    '  switch (entry.domFormat) {',
    '    case "dimension":',
    '      return formatDimension(value as HayateDimension);',
    '    case "dimension-list":',
    '      return formatDimensionList(value);',
    '    case "shadow-list":',
    '      return formatShadowList(value);',
    '    case "px":',
    '      return `${value}px`;',
    '    case "ms":',
    '      return `${value}ms`;',
    '    case "grid-placement":',
    '      return formatGridPlacement(value);',
    '    case "enum":',
    '      return entry.enumCss?.[value as string] ?? String(value);',
    '    case "integer":',
    '    case "number":',
    '    case "color":',
    '    case "string":',
    '      return String(value);',
    '    default: {',
    '      const _exhaustive: never = entry.domFormat;',
    '      throw new Error(`unsupported dom format ${_exhaustive}`);',
    '    }',
    '  }',
    '}',
    '',
    'export function applyDomExtras(',
    '  style: Record<string, string>,',
    '  entry: CatalogEntry,',
    '  value: unknown,',
    '): void {',
    '  if (!entry.domExtras) return;',
    '  for (const extra of entry.domExtras) {',
    '    const n = typeof value === "number" ? value : Number(value);',
    '    style[extra.cssName] = n > 0 ? extra.whenPositive : extra.whenZero;',
    '  }',
    '}',
    '',
  ];

  writeFileSync(tsOutPath, tsLines.join('\n'), 'utf8');
}
