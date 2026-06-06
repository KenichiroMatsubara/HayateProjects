import type { EventKind, InteractionEvent } from '@tsubame/renderer-protocol';
import { asElementId } from '@tsubame/renderer-protocol';
import { EVENT_KIND, type EventPayload, parseEvent } from './protocol.js';

/**
 * EventKinds that Hayate Document Runtime can register listeners for.
 * `change` is not yet wired on the Hayate side.
 */
export const HAYATE_LISTENER_KIND: Partial<Record<EventKind, number>> = {
  click: EVENT_KIND.CLICK,
  focus: EVENT_KIND.FOCUS,
  blur: EVENT_KIND.BLUR,
  input: EVENT_KIND.TEXT_INPUT,
  keydown: EVENT_KIND.KEY_DOWN,
  'hover-enter': EVENT_KIND.HOVER_ENTER,
  'hover-leave': EVENT_KIND.HOVER_LEAVE,
};

/**
 * Hayate event kinds that are intentionally not forwarded to host handlers.
 * Bubble path and scroll offset are handled in hayate-core (ADR-0053).
 */
const IGNORED_KINDS: ReadonlySet<EventPayload['kind']> = new Set([
  'composition_start',
  'composition_update',
  'composition_end',
  'scroll',
  'resize',
  'active_start',
  'active_end',
  'pointer_move',
  'fetch_font',
]);

export interface EventDelivery {
  listenerId: number;
  event: EventPayload;
}

/** Decode one Hayate `poll_events()` delivery row: `[listener_id, kind, ...fields]`. */
export function parseDelivery(row: unknown[]): EventDelivery {
  const listenerId = row[0] as number;
  const event = parseEvent(row.slice(1) as unknown[]);
  return { listenerId, event };
}

/** Map a parsed Hayate event payload to a Tsubame {@link InteractionEvent}, if deliverable. */
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
