import type { ElementId, ElementKind } from './element.js';
import type { StylePatch } from './style.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';

/**
 * Tsubame renderer/adaptor boundary.
 *
 * Adapters build an element tree, apply style patches, and register interaction
 * handlers through this interface without depending on a concrete DOM/Canvas
 * implementation.
 */
export interface IRenderer {
  createElement(kind: ElementKind): ElementId;
  setRoot(id: ElementId): void;
  appendChild(parent: ElementId, child: ElementId): void;
  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void;
  removeChild(parent: ElementId, child: ElementId): void;
  setStyle(id: ElementId, style: StylePatch): void;
  setText(id: ElementId, text: string): void;

  /**
   * Applies non-style props such as `src`, `value`, `placeholder`, or
   * `disabled`. Renderers may ignore unsupported props.
   */
  setProperty(id: ElementId, name: string, value: unknown): void;

  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe;

  resize(width: number, height: number): void;
}
