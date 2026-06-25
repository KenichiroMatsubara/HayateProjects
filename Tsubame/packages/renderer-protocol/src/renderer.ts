import type { ElementId, ElementKind } from './element.js';
import type { PseudoStyleKey, PseudoStylePatch } from './pseudo-style.js';
import type { StylePatch } from './style.js';
import type { ViewportCondition } from './viewport-condition.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';

/**
 * Tsubame の renderer/adaptor 境界。
 *
 * adapter はこのインターフェース越しに、具体的な DOM/Canvas 実装に依存せず、
 * 要素ツリーの構築・スタイルパッチの適用・インタラクションハンドラの登録を行う。
 */
export interface IRenderer {
  createElement(kind: ElementKind): ElementId;
  setRoot(id: ElementId): void;
  appendChild(parent: ElementId, child: ElementId): void;
  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void;
  removeChild(parent: ElementId, child: ElementId): void;
  setStyle(id: ElementId, style: StylePatch): void;
  /** Hayate CSS の擬似クラスブロック（`:hover` / `:active` / `:focus`）。 */
  setPseudoStyle(id: ElementId, pseudo: PseudoStyleKey, style: StylePatch): void;
  /** ビューポート条件付きのスタイル上書き。プロパティごとに 1 バリアント（ADR-0081）。 */
  setStyleVariant(id: ElementId, condition: ViewportCondition, style: StylePatch): void;
  setText(id: ElementId, text: string): void;

  /**
   * 閉じたセマンティックプロップ（`value` / `placeholder` / `disabled` / `src`）を適用する。
   * 未知の名前は throw すること（ADR-0071）。`aria-*` は first-class API のみを使う。
   */
  setProperty(id: ElementId, name: string, value: unknown): void;

  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe;
}
