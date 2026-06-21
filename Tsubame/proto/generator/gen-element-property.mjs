import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec } from '@hayate/protocol-spec/load';

const outDir = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../packages/renderer-protocol/src/generated',
);
const outPath = join(outDir, 'element-property.ts');

/**
 * Coercion strategies (the closed `coerce` vocabulary in element_properties.json).
 * Each maps a raw `setProperty` value to a coerced op payload. `field`/`tsType`
 * derive the op-payload shape; `expr(field)` is the coercion expression, written
 * exactly once here so DOM and Canvas can never drift (issue #235 / ADR-0008).
 */
const COERCE = {
  'stringify-nullable': {
    field: () => 'text',
    tsType: 'string',
    expr: () => `value == null ? '' : String(value)`,
  },
  'string-or-empty': {
    field: () => 'text',
    tsType: 'string',
    expr: () => `typeof value === 'string' ? value : ''`,
  },
  boolean: {
    field: (opKind) => opKind,
    tsType: 'boolean',
    expr: () => `Boolean(value)`,
  },
  // ADR-0108 closed `user-select` vocabulary. `text` / `contains` are
  // selectable, `none` excludes the subtree; an unknown value falls back to the
  // selectable default (`text`). The op carries the value under `value`.
  'user-select': {
    field: () => 'value',
    tsType: "'text' | 'none' | 'contains'",
    expr: () => `value === 'none' || value === 'contains' ? value : 'text'`,
  },
};

/**
 * Build the spec-derived prop-op model from a loaded protocol spec. Pure (no I/O)
 * so the generated dispatch can be tested as a single source against the spec.
 */
export function elementPropertyModel(proto) {
  const entries = proto.element_properties ?? [];
  return {
    names: entries.map((e) => e.name),
    ops: entries.map((e) => {
      const strategy = COERCE[e.coerce];
      if (!strategy) {
        throw new Error(
          `element_properties: "${e.name}" has unknown coerce strategy "${e.coerce}"`,
        );
      }
      return {
        kind: e.opKind,
        field: strategy.field(e.opKind),
        tsType: strategy.tsType,
      };
    }),
    cases: entries.map((e) => {
      const strategy = COERCE[e.coerce];
      return {
        name: e.name,
        kind: e.opKind,
        field: strategy.field(e.opKind),
        expr: strategy.expr(strategy.field(e.opKind)),
      };
    }),
  };
}

/** Render the spec-derived prop-op contract as a TypeScript module. */
export function renderElementProperty(model) {
  const unionMembers = model.ops.map(
    (op) => `  | { kind: '${op.kind}'; ${op.field}: ${op.tsType} }`,
  );
  const coerceCases = model.cases.map(
    (c) => `    case '${c.name}':\n      return { kind: '${c.kind}', ${c.field}: ${c.expr} };`,
  );

  return [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec（element_properties）',
    '',
    '/** 閉じた要素プロパティ語彙（ADR-0071）。`aria-*` は専用 API のみを使用する。 */',
    `export const ELEMENT_PROPERTY_NAMES = ${JSON.stringify(model.names)} as const;`,
    '',
    'export type ElementPropertyName = (typeof ELEMENT_PROPERTY_NAMES)[number];',
    '',
    '/**',
    ' * `setProperty(name, value)` 呼び出しを変換した、レンダラー非依存の結果',
    ' *（issue #235）。判別子は論理的な効果を表す。DOM と Canvas の',
    ' * レンダラーは*同一*の変換済みペイロードをそれぞれの媒体に適用するため、',
    ' * 型変換のエッジケース（null の消去、文字列化、`Boolean()`）は',
    ' * ただ一箇所に存在し、2つのレンダラー間でずれることがない。',
    ' */',
    'export type ElementPropertyOp =',
    ...unionMembers,
    '  ;',
    '',
    '/** 既知の要素プロパティと生の値を、共有された意味論へ変換する。 */',
    'export function coerceElementProperty(',
    '  name: ElementPropertyName,',
    '  value: unknown,',
    '): ElementPropertyOp {',
    '  switch (name) {',
    ...coerceCases,
    '  }',
    '}',
    '',
    '/**',
    ' * op-kind をキーとする効果ハンドラ。各レンダラーは自身の媒体（DOM 変更 /',
    ' * Canvas のキュー投入）への書き込みでこれらを埋める。アダプターに委ねられる',
    ' * *唯一*の部分（ADR-0008）。新しい op-kind はこのマップを広げるため、各レンダラーは',
    ' * 新しいハンドラを供給しなければ型チェックに失敗する。',
    ' */',
    'export type ElementPropertyEffects<R> = {',
    "  [Op in ElementPropertyOp as Op['kind']]: (op: Op) => R;",
    '};',
    '',
    '/**',
    ' * 共有の prop-op ディスパッチ（ADR-0008）。変換済みの op を効果ハンドラへ振り分ける。',
    ' * op-kind の分岐はここに一度だけ存在し、レンダラーが再実装することはない。',
    ' */',
    'export function dispatchElementPropertyOp<R>(',
    '  op: ElementPropertyOp,',
    '  effects: ElementPropertyEffects<R>,',
    '): R {',
    '  const handler = effects[op.kind] as (op: ElementPropertyOp) => R;',
    '  return handler(op);',
    '}',
    '',
  ].join('\n');
}

export function generateElementProperty() {
  const proto = loadProtocolSpec();
  const model = elementPropertyModel(proto);
  mkdirSync(outDir, { recursive: true });
  writeFileSync(outPath, renderElementProperty(model), 'utf8');
}
