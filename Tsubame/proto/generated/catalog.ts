// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @hayate/protocol-spec

import type { HayateDimension, HayateShadow } from '@tsubame/renderer-protocol';

export type WireKind = 'color' | 'dimension' | 'dimensionList' | 'shadowList' | 'display' | 'flexDirection' | 'flexWrap' | 'alignItems' | 'alignSelf' | 'alignContent' | 'justifyContent' | 'fontStyle' | 'textDecoration' | 'borderStyle' | 'cursor' | 'overflow' | 'textOverflow' | 'position' | 'transitionTiming' | 'f32' | 'u32' | 'zIndex' | 'fontFamily';
export type DomFormat = 'dimension' | 'dimension-list' | 'shadow-list' | 'px' | 'ms' | 'number' | 'integer' | 'color' | 'enum' | 'string';

export interface DomExtra {
  readonly cssName: string;
  readonly cssProperty: string;
  readonly whenPositive: string;
  readonly whenZero: string;
}

export interface CatalogEntry {
  readonly patchKey: string;
  readonly tag: number;
  readonly unsetKind: number | null;
  readonly wireKind: WireKind;
  readonly domFormat: DomFormat;
  readonly cssName: string;
  readonly cssProperty: string;
  readonly targets: readonly ("packet" | "css")[];
  readonly domExtras?: readonly DomExtra[];
}

export const HAYATE_CSS_CATALOG: readonly CatalogEntry[] = [
  {
    "patchKey": "backgroundColor",
    "tag": 0,
    "unsetKind": null,
    "wireKind": "color",
    "domFormat": "color",
    "cssName": "backgroundColor",
    "cssProperty": "background-color",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "opacity",
    "tag": 1,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "number",
    "cssName": "opacity",
    "cssProperty": "opacity",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "borderRadius",
    "tag": 2,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "px",
    "cssName": "borderRadius",
    "cssProperty": "border-radius",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "borderWidth",
    "tag": 3,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "px",
    "cssName": "borderWidth",
    "cssProperty": "border-width",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "borderColor",
    "tag": 4,
    "unsetKind": null,
    "wireKind": "color",
    "domFormat": "color",
    "cssName": "borderColor",
    "cssProperty": "border-color",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "width",
    "tag": 5,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "width",
    "cssProperty": "width",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "height",
    "tag": 6,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "height",
    "cssProperty": "height",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "minWidth",
    "tag": 7,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "minWidth",
    "cssProperty": "min-width",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "minHeight",
    "tag": 8,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "minHeight",
    "cssProperty": "min-height",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "maxWidth",
    "tag": 9,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "maxWidth",
    "cssProperty": "max-width",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "maxHeight",
    "tag": 10,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "maxHeight",
    "cssProperty": "max-height",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "display",
    "tag": 11,
    "unsetKind": null,
    "wireKind": "display",
    "domFormat": "enum",
    "cssName": "display",
    "cssProperty": "display",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "flexDirection",
    "tag": 12,
    "unsetKind": null,
    "wireKind": "flexDirection",
    "domFormat": "enum",
    "cssName": "flexDirection",
    "cssProperty": "flex-direction",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "alignItems",
    "tag": 13,
    "unsetKind": null,
    "wireKind": "alignItems",
    "domFormat": "enum",
    "cssName": "alignItems",
    "cssProperty": "align-items",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "justifyContent",
    "tag": 14,
    "unsetKind": null,
    "wireKind": "justifyContent",
    "domFormat": "enum",
    "cssName": "justifyContent",
    "cssProperty": "justify-content",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "gap",
    "tag": 15,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "gap",
    "cssProperty": "gap",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "padding",
    "tag": 16,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "padding",
    "cssProperty": "padding",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "paddingTop",
    "tag": 17,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "paddingTop",
    "cssProperty": "padding-top",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "paddingRight",
    "tag": 18,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "paddingRight",
    "cssProperty": "padding-right",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "paddingBottom",
    "tag": 19,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "paddingBottom",
    "cssProperty": "padding-bottom",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "paddingLeft",
    "tag": 20,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "paddingLeft",
    "cssProperty": "padding-left",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "margin",
    "tag": 21,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "margin",
    "cssProperty": "margin",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "marginTop",
    "tag": 22,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "marginTop",
    "cssProperty": "margin-top",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "marginRight",
    "tag": 23,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "marginRight",
    "cssProperty": "margin-right",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "marginBottom",
    "tag": 24,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "marginBottom",
    "cssProperty": "margin-bottom",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "marginLeft",
    "tag": 25,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "marginLeft",
    "cssProperty": "margin-left",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "fontSize",
    "tag": 26,
    "unsetKind": 1,
    "wireKind": "f32",
    "domFormat": "px",
    "cssName": "fontSize",
    "cssProperty": "font-size",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "color",
    "tag": 27,
    "unsetKind": 0,
    "wireKind": "color",
    "domFormat": "color",
    "cssName": "color",
    "cssProperty": "color",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "zIndex",
    "tag": 28,
    "unsetKind": null,
    "wireKind": "zIndex",
    "domFormat": "integer",
    "cssName": "zIndex",
    "cssProperty": "z-index",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "fontFamily",
    "tag": 29,
    "unsetKind": 2,
    "wireKind": "fontFamily",
    "domFormat": "string",
    "cssName": "fontFamily",
    "cssProperty": "font-family",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "flexGrow",
    "tag": 30,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "number",
    "cssName": "flexGrow",
    "cssProperty": "flex-grow",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "fontWeight",
    "tag": 31,
    "unsetKind": 3,
    "wireKind": "f32",
    "domFormat": "number",
    "cssName": "fontWeight",
    "cssProperty": "font-weight",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "fontStyle",
    "tag": 32,
    "unsetKind": null,
    "wireKind": "fontStyle",
    "domFormat": "enum",
    "cssName": "fontStyle",
    "cssProperty": "font-style",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "textDecoration",
    "tag": 33,
    "unsetKind": null,
    "wireKind": "textDecoration",
    "domFormat": "enum",
    "cssName": "textDecoration",
    "cssProperty": "text-decoration",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "defaultColor",
    "tag": 34,
    "unsetKind": null,
    "wireKind": "color",
    "domFormat": "color",
    "cssName": "color",
    "cssProperty": "color",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "defaultFontFamily",
    "tag": 35,
    "unsetKind": null,
    "wireKind": "fontFamily",
    "domFormat": "string",
    "cssName": "fontFamily",
    "cssProperty": "font-family",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "defaultFontSize",
    "tag": 36,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "px",
    "cssName": "fontSize",
    "cssProperty": "font-size",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "defaultFontWeight",
    "tag": 37,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "number",
    "cssName": "fontWeight",
    "cssProperty": "font-weight",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "gridTemplateColumns",
    "tag": 38,
    "unsetKind": null,
    "wireKind": "dimensionList",
    "domFormat": "dimension-list",
    "cssName": "gridTemplateColumns",
    "cssProperty": "grid-template-columns",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "gridTemplateRows",
    "tag": 39,
    "unsetKind": null,
    "wireKind": "dimensionList",
    "domFormat": "dimension-list",
    "cssName": "gridTemplateRows",
    "cssProperty": "grid-template-rows",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "flexShrink",
    "tag": 40,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "number",
    "cssName": "flexShrink",
    "cssProperty": "flex-shrink",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "flexBasis",
    "tag": 41,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "flexBasis",
    "cssProperty": "flex-basis",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "alignSelf",
    "tag": 42,
    "unsetKind": null,
    "wireKind": "alignSelf",
    "domFormat": "enum",
    "cssName": "alignSelf",
    "cssProperty": "align-self",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "alignContent",
    "tag": 43,
    "unsetKind": null,
    "wireKind": "alignContent",
    "domFormat": "enum",
    "cssName": "alignContent",
    "cssProperty": "align-content",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "flexWrap",
    "tag": 44,
    "unsetKind": null,
    "wireKind": "flexWrap",
    "domFormat": "enum",
    "cssName": "flexWrap",
    "cssProperty": "flex-wrap",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "borderStyle",
    "tag": 45,
    "unsetKind": null,
    "wireKind": "borderStyle",
    "domFormat": "enum",
    "cssName": "borderStyle",
    "cssProperty": "border-style",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "cursor",
    "tag": 46,
    "unsetKind": null,
    "wireKind": "cursor",
    "domFormat": "enum",
    "cssName": "cursor",
    "cssProperty": "cursor",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "position",
    "tag": 47,
    "unsetKind": null,
    "wireKind": "position",
    "domFormat": "enum",
    "cssName": "position",
    "cssProperty": "position",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "top",
    "tag": 48,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "top",
    "cssProperty": "top",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "left",
    "tag": 49,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "left",
    "cssProperty": "left",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "right",
    "tag": 50,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "right",
    "cssProperty": "right",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "bottom",
    "tag": 51,
    "unsetKind": null,
    "wireKind": "dimension",
    "domFormat": "dimension",
    "cssName": "bottom",
    "cssProperty": "bottom",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "overflow",
    "tag": 52,
    "unsetKind": null,
    "wireKind": "overflow",
    "domFormat": "enum",
    "cssName": "overflow",
    "cssProperty": "overflow",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "maxLines",
    "tag": 53,
    "unsetKind": null,
    "wireKind": "u32",
    "domFormat": "integer",
    "cssName": "WebkitLineClamp",
    "cssProperty": "-webkit-line-clamp",
    "targets": [
      "packet",
      "css"
    ],
    "domExtras": [
      {
        "cssName": "display",
        "cssProperty": "display",
        "whenPositive": "-webkit-box",
        "whenZero": "block"
      },
      {
        "cssName": "WebkitBoxOrient",
        "cssProperty": "-webkit-box-orient",
        "whenPositive": "vertical",
        "whenZero": "horizontal"
      },
      {
        "cssName": "overflow",
        "cssProperty": "overflow",
        "whenPositive": "hidden",
        "whenZero": "visible"
      }
    ]
  },
  {
    "patchKey": "textOverflow",
    "tag": 54,
    "unsetKind": null,
    "wireKind": "textOverflow",
    "domFormat": "enum",
    "cssName": "textOverflow",
    "cssProperty": "text-overflow",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "transitionDuration",
    "tag": 55,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "ms",
    "cssName": "transitionDuration",
    "cssProperty": "transition-duration",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "transitionTiming",
    "tag": 56,
    "unsetKind": null,
    "wireKind": "transitionTiming",
    "domFormat": "enum",
    "cssName": "transitionTimingFunction",
    "cssProperty": "transition-timing-function",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "boxShadow",
    "tag": 57,
    "unsetKind": null,
    "wireKind": "shadowList",
    "domFormat": "shadow-list",
    "cssName": "boxShadow",
    "cssProperty": "box-shadow",
    "targets": [
      "packet",
      "css"
    ]
  },
  {
    "patchKey": "aspectRatio",
    "tag": 58,
    "unsetKind": null,
    "wireKind": "f32",
    "domFormat": "number",
    "cssName": "aspectRatio",
    "cssProperty": "aspect-ratio",
    "targets": [
      "packet",
      "css"
    ]
  }
];

export const CATALOG_BY_KEY: Readonly<Record<string, CatalogEntry>> = Object.fromEntries(
  HAYATE_CSS_CATALOG.map((e) => [e.patchKey, e]),
);

export const CATALOG_BY_TAG: Readonly<Record<number, CatalogEntry>> = Object.fromEntries(
  HAYATE_CSS_CATALOG.map((e) => [e.tag, e]),
);

export interface StyleEncodeEntry {
  readonly key: string;
  readonly tag: number;
  readonly kind: WireKind;
}

export const STYLE_ENCODE_META: readonly StyleEncodeEntry[] = HAYATE_CSS_CATALOG.filter(
  (e) => e.targets.includes("packet"),
).map((e) => ({ key: e.patchKey, tag: e.tag, kind: e.wireKind }));

export const INHERITED_UNSET: Partial<Record<string, number>> = Object.fromEntries(
  HAYATE_CSS_CATALOG.filter((e): e is CatalogEntry & { unsetKind: number } => e.unsetKind !== null)
    .map((e) => [e.patchKey, e.unsetKind]),
);

function formatDimension(value: HayateDimension): string {
  return typeof value === "number" ? `${value}px` : value;
}

function formatDimensionList(value: unknown): string {
  if (!Array.isArray(value)) {
    throw new Error("DOMRenderer: grid track list must be an array");
  }
  return value.map((item) => formatDimension(item as HayateDimension)).join(" ");
}

function formatShadow(shadow: HayateShadow): string {
  const parts = [];
  if (shadow.inset) parts.push("inset");
  parts.push(`${shadow.offsetX}px`, `${shadow.offsetY}px`, `${shadow.blur}px`, `${shadow.spread}px`, shadow.color);
  return parts.join(" ");
}

function formatShadowList(value: unknown): string {
  if (!Array.isArray(value)) {
    throw new Error("DOMRenderer: box-shadow must be an array");
  }
  if (value.length === 0) return "none";
  return value.map((item) => formatShadow(item as HayateShadow)).join(", ");
}

export function formatDomCSSValue(entry: CatalogEntry, value: unknown): string {
  switch (entry.domFormat) {
    case "dimension":
      return formatDimension(value as HayateDimension);
    case "dimension-list":
      return formatDimensionList(value);
    case "shadow-list":
      return formatShadowList(value);
    case "px":
      return `${value}px`;
    case "ms":
      return `${value}ms`;
    case "integer":
    case "number":
    case "color":
    case "enum":
    case "string":
      return String(value);
    default: {
      const _exhaustive: never = entry.domFormat;
      throw new Error(`unsupported dom format ${_exhaustive}`);
    }
  }
}

export function applyDomExtras(
  style: Record<string, string>,
  entry: CatalogEntry,
  value: unknown,
): void {
  if (!entry.domExtras) return;
  for (const extra of entry.domExtras) {
    const n = typeof value === "number" ? value : Number(value);
    style[extra.cssName] = n > 0 ? extra.whenPositive : extra.whenZero;
  }
}
