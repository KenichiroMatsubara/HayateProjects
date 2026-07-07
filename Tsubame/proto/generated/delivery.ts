// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @hayate/protocol-spec（event_kinds の wireRole / adapterTier / interactionKind）

import type { EventKind, InteractionEvent } from '@tsubame/renderer-protocol';
import { asElementId } from '@tsubame/renderer-protocol';
import { EVENT_KIND, type EventPayload, parseEvent } from './protocol.js';

/** Hayate の `register_listener` で登録可能な EventKind（adapterTier: forward）。 */
export const HAYATE_LISTENER_KIND: Partial<Record<EventKind, number>> = {
  'click': EVENT_KIND.CLICK,
  'focus': EVENT_KIND.FOCUS,
  'blur': EVENT_KIND.BLUR,
  'input': EVENT_KIND.TEXT_INPUT,
  'hover-enter': EVENT_KIND.HOVER_ENTER,
  'hover-leave': EVENT_KIND.HOVER_LEAVE,
  'keydown': EVENT_KIND.KEY_DOWN,
};

/** adapterTier が deferred の Hayate ワイヤー種別（scroll, composition_*, …）。 */
export const HAYATE_DEFERRED_LISTENER_KIND: Readonly<Record<string, number>> = {
  'composition_start': EVENT_KIND.COMPOSITION_START,
  'composition_update': EVENT_KIND.COMPOSITION_UPDATE,
  'composition_end': EVENT_KIND.COMPOSITION_END,
  'scroll': EVENT_KIND.SCROLL,
};

const IGNORED_KINDS: ReadonlySet<EventPayload['kind']> = new Set([
  'composition_start',
  'composition_update',
  'composition_end',
  'scroll',
  'resize',
  'active_end',
  'active_start',
  'pointer_move',
  'fetch_font',
  'selection_change',
  'layout_resize',
]);

export interface EventDelivery {
  listenerId: number;
  event: EventPayload;
}

/** Hayate の `poll_events()` の配信行 `[listener_id, kind, ...fields]` を1件デコードする。 */
export function parseDelivery(row: unknown[]): EventDelivery {
  const listenerId = row[0] as number;
  const event = parseEvent(row.slice(1) as unknown[]);
  return { listenerId, event };
}

/** 解析済みの Hayate イベントペイロードを、配信可能なら Tsubame の {@link InteractionEvent} へ変換する。 */
export function toInteractionEvent(ev: EventPayload): InteractionEvent | null {
  if (IGNORED_KINDS.has(ev.kind)) return null;

  switch (ev.kind) {
    case 'click':
      return { kind: 'click', target: asElementId(ev.targetId) };
    case 'focus':
      return { kind: 'focus', target: asElementId(ev.targetId) };
    case 'blur':
      return { kind: 'blur', target: asElementId(ev.targetId) };
    case 'text_input':
      return { kind: 'input', target: asElementId(ev.targetId), value: ev.text };
    case 'hover_enter':
      return { kind: 'hover-enter', target: asElementId(ev.targetId) };
    case 'hover_leave':
      return { kind: 'hover-leave', target: asElementId(ev.targetId) };
    case 'key_down':
      return { kind: 'keydown', target: asElementId(ev.targetId), key: ev.key };
    default:
      return null;
  }
}
