import type { ElementId, EventHandler, EventKind } from '@tsubame/renderer-protocol';
import { asElementId } from '@tsubame/renderer-protocol';
import type { EventPayload } from './protocol.js';
import { parseEvent } from './protocol.js';

// ── Policy tables ─────────────────────────────────────────────────────────────

/**
 * EventKinds that bubble up through the element tree (ADR-0034).
 */
const BUBBLING_EVENTS: ReadonlySet<EventKind> = new Set([
  'click',
  'input',
  'change',
  'keydown',
]);

/**
 * Raw event kinds from Hayate that are explicitly ignored (no-op policy).
 * composition / scroll / resize / active / pointer_move / fetch_font
 * are not yet forwarded to handlers.
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

// ── Public types ──────────────────────────────────────────────────────────────

export interface InteractionStreamOptions {
  getParent(id: ElementId): ElementId | undefined;
  getHandlers(id: ElementId, kind: EventKind): Iterable<EventHandler> | undefined;
}

export interface InteractionStream {
  /** Decode Hayate `poll_events()` array-of-arrays and dispatch each entry. */
  dispatchRawEvents(events: unknown[]): void;
  /** Dispatch a single already-parsed {@link EventPayload}. */
  dispatchParsedEvent(ev: EventPayload): void;
  /**
   * Dispatch one event of a known {@link EventKind} starting at `hit`,
   * optionally bubbling through ancestors.
   */
  dispatchOne(kind: EventKind, hit: ElementId, detail?: { value?: string; key?: string }): void;
}

// ── Factory ───────────────────────────────────────────────────────────────────

export function createInteractionStream(options: InteractionStreamOptions): InteractionStream {
  const { getParent, getHandlers } = options;

  function dispatchOne(
    kind: EventKind,
    hit: ElementId,
    detail?: { value?: string; key?: string },
  ): void {
    const bubbles = BUBBLING_EVENTS.has(kind);
    let node: ElementId | undefined = hit;
    while (node !== undefined) {
      const handlers = getHandlers(node, kind);
      if (handlers !== undefined) {
        for (const handler of handlers) handler({ kind, target: node, ...detail });
      }
      if (!bubbles) break;
      node = getParent(node);
    }
  }

  function dispatchParsedEvent(ev: EventPayload): void {
    if (IGNORED_KINDS.has(ev.kind)) return;

    switch (ev.kind) {
      case 'click':
        dispatchOne('click', asElementId(ev.targetId));
        break;
      case 'focus':
        dispatchOne('focus', asElementId(ev.targetId));
        break;
      case 'blur':
        dispatchOne('blur', asElementId(ev.targetId));
        break;
      case 'text_input':
        dispatchOne('input', asElementId(ev.targetId), { value: ev.text });
        break;
      case 'hover_enter':
        dispatchOne('hover-enter', asElementId(ev.targetId));
        break;
      case 'hover_leave':
        dispatchOne('hover-leave', asElementId(ev.targetId));
        break;
      case 'key_down':
        dispatchOne('keydown', asElementId(ev.targetId), { key: ev.key });
        break;
      default:
        // Exhaustiveness guard: any new EventPayload kind that is neither
        // mapped above nor in IGNORED_KINDS falls here (no-op for now).
        break;
    }
  }

  function dispatchRawEvents(events: unknown[]): void {
    for (const entry of events) {
      const ev = parseEvent(entry as unknown[]);
      dispatchParsedEvent(ev);
    }
  }

  return { dispatchRawEvents, dispatchParsedEvent, dispatchOne };
}
