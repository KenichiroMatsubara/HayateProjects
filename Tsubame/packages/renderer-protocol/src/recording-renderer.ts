import { asElementId, type ElementId, type ElementKind } from './element.js';
import type { PseudoStyleKey } from './pseudo-style.js';
import type { StylePatch } from './style.js';
import type { ViewportCondition } from './viewport-condition.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';
import type { IRenderer } from './renderer.js';

/**
 * Every {@link IRenderer} call captured as an ordered, discriminated record.
 * Tests read these instead of reaching into a concrete renderer's private DOM
 * or wire state — the seam is verified through the interface (Tsubame ADR-0008).
 */
export type RecordedCall =
  | { method: 'createElement'; id: ElementId; kind: ElementKind }
  | { method: 'setRoot'; id: ElementId }
  | { method: 'appendChild'; parent: ElementId; child: ElementId }
  | { method: 'insertBefore'; parent: ElementId; child: ElementId; before: ElementId }
  | { method: 'removeChild'; parent: ElementId; child: ElementId }
  | { method: 'setStyle'; id: ElementId; style: StylePatch }
  | { method: 'setPseudoStyle'; id: ElementId; pseudo: PseudoStyleKey; style: StylePatch }
  | { method: 'setStyleVariant'; id: ElementId; condition: ViewportCondition; style: StylePatch }
  | { method: 'setText'; id: ElementId; text: string }
  | { method: 'setProperty'; id: ElementId; name: string; value: unknown }
  | { method: 'addEventListener'; id: ElementId; event: EventKind }
  | { method: 'resize'; width: number; height: number };

/**
 * In-memory {@link IRenderer} that records each call. A second adapter behind
 * the Renderer Protocol so the gate seam (and any other cross-renderer
 * contract) can be exercised without a DOM or a Hayate WASM boundary.
 */
export class RecordingRenderer implements IRenderer {
  readonly calls: RecordedCall[] = [];
  private nextId = 1;

  createElement(kind: ElementKind): ElementId {
    const id = asElementId(this.nextId++);
    this.calls.push({ method: 'createElement', id, kind });
    return id;
  }

  setRoot(id: ElementId): void {
    this.calls.push({ method: 'setRoot', id });
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.calls.push({ method: 'appendChild', parent, child });
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.calls.push({ method: 'insertBefore', parent, child, before });
  }

  removeChild(parent: ElementId, child: ElementId): void {
    this.calls.push({ method: 'removeChild', parent, child });
  }

  setStyle(id: ElementId, style: StylePatch): void {
    this.calls.push({ method: 'setStyle', id, style });
  }

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    this.calls.push({ method: 'setPseudoStyle', id, pseudo, style });
  }

  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    this.calls.push({ method: 'setStyleVariant', id, condition, style });
  }

  setText(id: ElementId, text: string): void {
    this.calls.push({ method: 'setText', id, text });
  }

  setProperty(id: ElementId, name: string, value: unknown): void {
    this.calls.push({ method: 'setProperty', id, name, value });
  }

  addEventListener(id: ElementId, event: EventKind, _handler: EventHandler): Unsubscribe {
    this.calls.push({ method: 'addEventListener', id, event });
    return () => {};
  }

  resize(width: number, height: number): void {
    this.calls.push({ method: 'resize', width, height });
  }

  /** The last `setStyle` patch recorded for `id`, or `undefined` if none. */
  styleOf(id: ElementId): StylePatch | undefined {
    for (let i = this.calls.length - 1; i >= 0; i--) {
      const call = this.calls[i]!;
      if (call.method === 'setStyle' && call.id === id) return call.style;
    }
    return undefined;
  }
}
