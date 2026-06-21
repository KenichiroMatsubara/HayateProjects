import { writeFileSync, mkdirSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { loadProtocolSpec } from '@hayate/protocol-spec/load';

const outPath = join(dirname(fileURLToPath(import.meta.url)), '../generated/delivery.ts');

function interactionEventFields(ev) {
  const fields = [`kind: '${ev.interactionKind}'`, 'target: asElementId(ev.targetId)'];
  const paramNames = new Set((ev.params ?? []).map((p) => p.name));
  if (paramNames.has('text')) fields.push('value: ev.text');
  if (paramNames.has('key')) fields.push('key: ev.key');
  return fields;
}

export function generateDelivery() {
  const proto = loadProtocolSpec();
  const events = proto.event_kinds ?? [];

  const forwardMapped = events.filter(
    (ev) => ev.adapterTier === 'forward' && ev.interactionKind != null,
  );

  const deferredEntries = events.filter(
    (ev) =>
      ev.documentRuntime === true &&
      ev.adapterTier === 'deferred',
  );

  const ignoredNames = events
    .filter((ev) => ev.interactionKind == null)
    .map((ev) => `'${ev.name}'`);

  const listenerLines = forwardMapped.map((ev) => {
    const kind = ev.interactionKind;
    const constKey = ev.name.toUpperCase();
    return `  '${kind}': EVENT_KIND.${constKey},`;
  });

  const deferredLines = deferredEntries.map((ev) => {
    const constKey = ev.name.toUpperCase();
    return `  '${ev.name}': EVENT_KIND.${constKey},`;
  });

  const switchCases = forwardMapped.map((ev) => {
    const fields = interactionEventFields(ev);
    return `    case '${ev.name}':\n      return { ${fields.join(', ')} };`;
  });

  const lines = [
    '// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと',
    '// 生成元: @hayate/protocol-spec（event_kinds の wireRole / adapterTier / interactionKind）',
    '',
    "import type { EventKind, InteractionEvent } from '@tsubame/renderer-protocol';",
    "import { asElementId } from '@tsubame/renderer-protocol';",
    "import { EVENT_KIND, type EventPayload, parseEvent } from './protocol.js';",
    '',
    '/** Hayate の `register_listener` で登録可能な EventKind（adapterTier: forward）。 */',
    'export const HAYATE_LISTENER_KIND: Partial<Record<EventKind, number>> = {',
    ...listenerLines,
    '};',
    '',
    '/** adapterTier が deferred の Hayate ワイヤー種別（scroll, composition_*, …）。 */',
    'export const HAYATE_DEFERRED_LISTENER_KIND: Readonly<Record<string, number>> = {',
    ...deferredLines,
    '};',
    '',
    'const IGNORED_KINDS: ReadonlySet<EventPayload[\'kind\']> = new Set([',
    ...ignoredNames.map((n) => `  ${n},`),
    ']);',
    '',
    'export interface EventDelivery {',
    '  listenerId: number;',
    '  event: EventPayload;',
    '}',
    '',
    '/** Hayate の `poll_events()` の配信行 `[listener_id, kind, ...fields]` を1件デコードする。 */',
    'export function parseDelivery(row: unknown[]): EventDelivery {',
    '  const listenerId = row[0] as number;',
    '  const event = parseEvent(row.slice(1) as unknown[]);',
    '  return { listenerId, event };',
    '}',
    '',
    '/** 解析済みの Hayate イベントペイロードを、配信可能なら Tsubame の {@link InteractionEvent} へ変換する。 */',
    'export function toInteractionEvent(ev: EventPayload): InteractionEvent | null {',
    '  if (IGNORED_KINDS.has(ev.kind)) return null;',
    '',
    '  switch (ev.kind) {',
    ...switchCases,
    '    default:',
    '      return null;',
    '  }',
    '}',
    '',
  ];

  mkdirSync(dirname(outPath), { recursive: true });
  writeFileSync(outPath, lines.join('\n'), 'utf8');
}
