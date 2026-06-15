import type { ElementId, ElementKind } from './element.js';
import type { PseudoStyleKey } from './pseudo-style.js';
import type { StylePatch } from './style.js';
import type { ViewportCondition } from './viewport-condition.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';
import type { IRenderer } from './renderer.js';
import { gateTextLocalPatch } from './text-local-gate.js';

/**
 * Renderer decorator that applies the Style Channel gate **once, in front of**
 * the wrapped renderer (Tsubame ADR-0008). It learns each element's kind from
 * `createElement`, filters channel-1 text-local props out of every style-bearing
 * op, and forwards the already-gated patch downstream. Every renderer behind the
 * seam therefore receives the identical filtered patch — Semantics Parity holds
 * by construction, not by a per-renderer test, and a newly added renderer needs
 * no gate of its own.
 */
class GatingRenderer implements IRenderer {
  private readonly kinds = new Map<ElementId, ElementKind>();

  constructor(private readonly inner: IRenderer) {}

  createElement(kind: ElementKind): ElementId {
    const id = this.inner.createElement(kind);
    this.kinds.set(id, kind);
    return id;
  }

  setRoot(id: ElementId): void {
    this.inner.setRoot(id);
  }

  appendChild(parent: ElementId, child: ElementId): void {
    this.inner.appendChild(parent, child);
  }

  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void {
    this.inner.insertBefore(parent, child, before);
  }

  removeChild(parent: ElementId, child: ElementId): void {
    this.kinds.delete(child);
    this.inner.removeChild(parent, child);
  }

  setStyle(id: ElementId, style: StylePatch): void {
    this.inner.setStyle(id, this.gate(id, style));
  }

  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void {
    this.inner.setPseudoStyle(id, pseudo, this.gate(id, style));
  }

  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void {
    this.inner.setStyleVariant(id, condition, this.gate(id, style));
  }

  setText(id: ElementId, text: string): void {
    this.inner.setText(id, text);
  }

  setProperty(id: ElementId, name: string, value: unknown): void {
    this.inner.setProperty(id, name, value);
  }

  addEventListener(id: ElementId, event: EventKind, handler: EventHandler): Unsubscribe {
    return this.inner.addEventListener(id, event, handler);
  }

  resize(width: number, height: number): void {
    this.inner.resize(width, height);
  }

  /**
   * Filter text-local props the element's kind does not carry. An id with no
   * preceding `createElement` (kind unknown) passes through unchanged.
   */
  private gate(id: ElementId, style: StylePatch): StylePatch {
    const kind = this.kinds.get(id);
    return kind === undefined ? style : gateTextLocalPatch(kind, style);
  }
}

/** Wrap a renderer so the Style Channel gate is applied once before it (ADR-0008). */
export function withTextLocalGate(inner: IRenderer): IRenderer {
  return new GatingRenderer(inner);
}
