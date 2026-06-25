import { asElementId, type ElementId, type ElementKind } from './element.js';
import type { PseudoStyleKey } from './pseudo-style.js';
import type { StylePatch } from './style.js';
import type { ViewportCondition } from './viewport-condition.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';
import type { IRenderer } from './renderer.js';

/**
 * 各 {@link IRenderer} 呼び出しを、順序付きの判別可能レコードとして記録したもの。
 * テストは具象レンダラの private な DOM や wire 状態に踏み込まず、これを読む —
 * 継ぎ目はインターフェース越しに検証する（Tsubame ADR-0008）。
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
  | { method: 'removeEventListener'; id: ElementId; event: EventKind };

/**
 * 各呼び出しを記録するインメモリの {@link IRenderer}。Renderer Protocol の背後にある
 * 第2のアダプタで、ゲートの継ぎ目（や他のレンダラ間契約）を DOM や Hayate WASM 境界なしで
 * 検証できる。
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
    // 返す Unsubscribe を呼ぶと解除を記録する。リスナの差し替え／解除（旧購読を切って
    // から再登録する経路）を、具象レンダラの内部状態に踏み込まず IRenderer 境界の
    // 記録列だけで検証できるようにする（ADR-0008）。
    return () => {
      this.calls.push({ method: 'removeEventListener', id, event });
    };
  }

  /** `id` に対して記録された最後の `setStyle` パッチ。なければ `undefined`。 */
  styleOf(id: ElementId): StylePatch | undefined {
    for (let i = this.calls.length - 1; i >= 0; i--) {
      const call = this.calls[i]!;
      if (call.method === 'setStyle' && call.id === id) return call.style;
    }
    return undefined;
  }
}
