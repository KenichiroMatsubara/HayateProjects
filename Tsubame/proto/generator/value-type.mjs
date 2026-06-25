/** Closed vocabulary for style-tag wire/codec/DOM codegen (generator-internal). */

import { toCamelCase } from '@hayate/protocol-spec/load';

/**
 * @typedef {{ type: 'color' }} ColorValueType
 * @typedef {{ type: 'dimension' }} DimensionValueType
 * @typedef {{ type: 'scalar' }} ScalarValueType
 * @typedef {{ type: 'u32' }} U32ValueType
 * @typedef {{ type: 'enum', kind: string }} EnumValueType
 * @typedef {{ type: 'dimensionList' }} DimensionListValueType
 * @typedef {{ type: 'shadowList' }} ShadowListValueType
 * @typedef {{ type: 'fontFamily' }} FontFamilyValueType
 * @typedef {{ type: 'zIndex' }} ZIndexValueType
 * @typedef {ColorValueType | DimensionValueType | ScalarValueType | U32ValueType | EnumValueType | DimensionListValueType | ShadowListValueType | FontFamilyValueType | ZIndexValueType} ValueType
 */

const KNOWN_ENUM_KINDS = new Set([
  'display',
  'flex_direction',
  'flex_wrap',
  'align_items',
  'align_self',
  'align_content',
  'justify_content',
  'font_style',
  'text_decoration',
  'border_style',
  'cursor',
  'overflow',
  'text_overflow',
  'position',
  'transition_timing',
  'box_sizing',
  'grid_auto_flow',
]);

function enumKindFromEncodeFrom(encodeFrom) {
  const kind = encodeFrom.slice('enum:'.length);
  if (!KNOWN_ENUM_KINDS.has(kind)) {
    throw new Error(`unknown enum encodeFrom kind: ${kind}`);
  }
  return kind;
}

/**
 * Derive value type from spec fields only (`encodeFrom` + param type + `variable_length`).
 * @param {import('@hayate/protocol-spec/load').StyleTag} tag
 * @returns {import('./value-type.mjs').ValueType}
 */
export function classify(tag) {
  const encodeFrom = tag.encodeFrom;
  if (!encodeFrom) {
    throw new Error(`style tag ${tag.name} missing encodeFrom`);
  }
  const primaryParam = tag.params?.[0]?.type;

  switch (encodeFrom) {
    case 'css-color':
      return { type: 'color' };
    case 'dimension':
      return { type: 'dimension' };
    case 'f32':
      return { type: 'scalar' };
    case 'u32':
      return { type: 'u32' };
    case 'font-family': {
      if (!tag.variable_length) {
        throw new Error(`font-family tag ${tag.name} must set variable_length`);
      }
      if (primaryParam !== 'string') {
        throw new Error(`font-family tag ${tag.name} requires string param`);
      }
      return { type: 'fontFamily' };
    }
    case 'dimension-list': {
      if (!tag.variable_length) {
        throw new Error(`dimension-list tag ${tag.name} must set variable_length`);
      }
      return { type: 'dimensionList' };
    }
    case 'shadow-list': {
      if (!tag.variable_length) {
        throw new Error(`shadow-list tag ${tag.name} must set variable_length`);
      }
      return { type: 'shadowList' };
    }
    case 'z-index': {
      if (primaryParam !== 'i32') {
        throw new Error(`z-index tag ${tag.name} requires i32 param`);
      }
      return { type: 'zIndex' };
    }
    default: {
      if (encodeFrom.startsWith('enum:')) {
        return { type: 'enum', kind: enumKindFromEncodeFrom(encodeFrom) };
      }
      throw new Error(`unknown encodeFrom ${encodeFrom} for tag ${tag.name}`);
    }
  }
}

function enumKindToTsTypeName(kind) {
  return kind
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join('');
}

/** Catalog `WireKind` label for a classified value type. */
export function wireKind(valueType) {
  switch (valueType.type) {
    case 'color':
      return 'color';
    case 'dimension':
      return 'dimension';
    case 'scalar':
      return 'f32';
    case 'u32':
      return 'u32';
    case 'dimensionList':
      return 'dimensionList';
    case 'shadowList':
      return 'shadowList';
    case 'fontFamily':
      return 'fontFamily';
    case 'zIndex':
      return 'zIndex';
    case 'enum':
      return toCamelCase(valueType.kind);
    default: {
      const _exhaustive = valueType;
      throw new Error(`unsupported value type ${JSON.stringify(_exhaustive)}`);
    }
  }
}

/** TypeScript property type for a classified value type. */
export function tsType(valueType) {
  switch (valueType.type) {
    case 'color':
    case 'fontFamily':
      return 'string';
    case 'dimension':
      return 'HayateDimension';
    case 'scalar':
    case 'u32':
    case 'zIndex':
      return 'number';
    case 'dimensionList':
      return 'HayateDimension[]';
    case 'shadowList':
      return 'HayateShadow[]';
    case 'enum':
      return enumKindToTsTypeName(valueType.kind);
    default: {
      const _exhaustive = valueType;
      throw new Error(`unsupported value type ${JSON.stringify(_exhaustive)}`);
    }
  }
}

const ENUM_CONST_NAMES = {
  display: 'DISPLAY',
  flex_direction: 'FLEX_DIRECTION',
  flex_wrap: 'FLEX_WRAP',
  align_items: 'ALIGN_ITEMS',
  align_self: 'ALIGN_SELF',
  align_content: 'ALIGN_CONTENT',
  justify_content: 'JUSTIFY_CONTENT',
  font_style: 'FONT_STYLE',
  text_decoration: 'TEXT_DECORATION',
  border_style: 'BORDER_STYLE',
  cursor: 'CURSOR',
  overflow: 'OVERFLOW',
  text_overflow: 'TEXT_OVERFLOW',
  position: 'POSITION',
  transition_timing: 'TRANSITION_TIMING',
  box_sizing: 'BOX_SIZING',
  grid_auto_flow: 'GRID_AUTO_FLOW',
};

const ENUM_PATCH_LABELS = {
  display: 'display',
  flex_direction: 'flexDirection',
  flex_wrap: 'flexWrap',
  align_items: 'alignItems',
  align_self: 'alignSelf',
  align_content: 'alignContent',
  justify_content: 'justifyContent',
  font_style: 'fontStyle',
  text_decoration: 'textDecoration',
  border_style: 'borderStyle',
  cursor: 'cursor',
  overflow: 'overflow',
  text_overflow: 'textOverflow',
  position: 'position',
  transition_timing: 'transitionTiming',
  box_sizing: 'boxSizing',
  grid_auto_flow: 'gridAutoFlow',
};

/** Lines for a per-tag style encoder function body (excluding signature). */
export function styleEncoderLines(valueType, tagName, patchKey) {
  const fnName = `encode_${patchKey}`;
  switch (valueType.type) {
    case 'color':
      return [
        `function ${fnName}(out: number[], value: string): void {`,
        `  const c = parseColor(value);`,
        `  out.push(TAG.${tagName}, c.r, c.g, c.b, c.a);`,
        '}',
      ];
    case 'dimension':
      return [
        `function ${fnName}(out: number[], value: import('@tsubame/renderer-protocol').HayateDimension): void {`,
        `  const d = parseDimension(value);`,
        `  out.push(TAG.${tagName}, d.value, UNIT_CODE[d.unit]!);`,
        '}',
      ];
    case 'scalar':
      return [
        `function ${fnName}(out: number[], value: unknown): void {`,
        `  out.push(TAG.${tagName}, finiteNumber('${patchKey}', value));`,
        '}',
      ];
    case 'u32':
      return [
        `function ${fnName}(out: number[], value: unknown): void {`,
        `  out.push(TAG.${tagName}, finiteInteger('${patchKey}', value));`,
        '}',
      ];
    case 'zIndex':
      return [
        `function ${fnName}(out: number[], value: unknown): void {`,
        `  out.push(TAG.${tagName}, finiteInteger('${patchKey}', value));`,
        '}',
      ];
    case 'fontFamily':
      return [
        `function ${fnName}(out: number[], value: string): void {`,
        `  const bytes = new TextEncoder().encode(value);`,
        `  out.push(TAG.${tagName}, bytes.length);`,
        `  for (const byte of bytes) out.push(byte);`,
        '}',
      ];
    case 'dimensionList':
      return [
        `function ${fnName}(out: number[], value: import('@tsubame/renderer-protocol').HayateDimension[]): void {`,
        `  if (!Array.isArray(value)) {`,
        `    throw new Error(\`CanvasRenderer: "${patchKey}" must be an array of dimensions\`);`,
        `  }`,
        `  out.push(TAG.${tagName}, value.length);`,
        `  for (const item of value) {`,
        `    const d = parseDimension(item);`,
        `    out.push(d.value, UNIT_CODE[d.unit]!);`,
        `  }`,
        '}',
      ];
    case 'shadowList':
      return [
        `function ${fnName}(out: number[], value: import('@tsubame/renderer-protocol').HayateShadow[]): void {`,
        `  if (!Array.isArray(value)) {`,
        `    throw new Error(\`CanvasRenderer: "${patchKey}" must be an array of shadows\`);`,
        `  }`,
        `  out.push(TAG.${tagName}, value.length);`,
        `  for (const item of value) {`,
        `    const c = parseColor(item.color);`,
        `    out.push(`,
        `      finiteNumber('${patchKey}.offsetX', item.offsetX),`,
        `      finiteNumber('${patchKey}.offsetY', item.offsetY),`,
        `      finiteNumber('${patchKey}.blur', item.blur),`,
        `      finiteNumber('${patchKey}.spread', item.spread),`,
        `      c.r, c.g, c.b, c.a,`,
        `      item.inset ? 1 : 0,`,
        `    );`,
        `  }`,
        '}',
      ];
    case 'enum': {
      const constName = ENUM_CONST_NAMES[valueType.kind];
      const label = ENUM_PATCH_LABELS[valueType.kind];
      if (!constName || !label) {
        throw new Error(`unknown enum kind: ${valueType.kind}`);
      }
      return [
        `function ${fnName}(out: number[], value: string): void {`,
        `  const code = ${constName}_CODE[value];`,
        `  if (code === undefined) throw new Error(\`CanvasRenderer: unsupported ${label} "\${value}"\`);`,
        `  out.push(TAG.${tagName}, code);`,
        '}',
      ];
    }
    default: {
      const _exhaustive = valueType;
      throw new Error(`unsupported value type ${JSON.stringify(_exhaustive)}`);
    }
  }
}
