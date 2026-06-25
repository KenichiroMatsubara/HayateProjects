import type { ElementId, ElementKind } from './element.js';
import type { PseudoStyleKey } from './pseudo-style.js';
import type { StylePatch } from './style.js';
import type { ViewportCondition } from './viewport-condition.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';
import type { IRenderer } from './renderer.js';
import { gateTextLocalPatch } from './text-local-gate.js';

/**
 * ラップした renderer の**手前で一度だけ** Style Channel ゲートを適用する
 * renderer デコレータ（Tsubame ADR-0008）。各要素の kind を `createElement` から
 * 学習し、スタイルを伴う全 op から channel-1 の text-local プロップを除去して、
 * ゲート済みパッチを下流へ渡す。継ぎ目より後ろの全 renderer は同一のフィルタ済み
 * パッチを受け取るため、Semantics Parity は renderer ごとのテストではなく構成上
 * 成立し、新たに追加した renderer は独自のゲートを持つ必要がない。
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

  /**
   * 要素の kind が持たない text-local プロップを除去する。先行する
   * `createElement` がない id（kind 不明）はそのまま通す。
   */
  private gate(id: ElementId, style: StylePatch): StylePatch {
    const kind = this.kinds.get(id);
    return kind === undefined ? style : gateTextLocalPatch(kind, style);
  }
}

/** renderer をラップし、その手前で Style Channel ゲートを一度だけ適用する（ADR-0008）。 */
export function withTextLocalGate(inner: IRenderer): IRenderer {
  return new GatingRenderer(inner);
}
