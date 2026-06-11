import type { ElementId, ElementKind } from './element.js';
import type { PseudoStyleKey, PseudoStylePatch } from './pseudo-style.js';
import type { StylePatch } from './style.js';
import type { ViewportCondition } from './viewport-condition.js';
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
  /** Hayate CSS pseudo-class block (`:hover` / `:active` / `:focus`). */
  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void;
  /** Viewport-conditional style override, one variant per property (ADR-0081). */
  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void;
  setText(id: ElementId, text: string): void;

  /**
   * Applies closed semantic props (`value` / `placeholder` / `disabled` / `src`).
   * Unknown names must throw (ADR-0071). `aria-*` uses first-class APIs only.
   */
  setProperty(id: ElementId, name: string, value: unknown): void;

  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe;

  resize(width: number, height: number): void;
}
