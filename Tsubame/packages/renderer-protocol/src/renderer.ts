import type { ElementId, ElementKind } from './element.js';
import type { StylePatch } from './style.js';
import type { EventHandler, EventKind, Unsubscribe } from './event.js';

/**
 * Tsubame と Tsubame Adapter の間の境界インターフェース。
 *
 * element の作成・ツリー操作・スタイル設定・イベント購読を抽象化する。
 * DOM Renderer と Canvas Renderer の二つの実装を持ち、Adapter は
 * このインターフェースを通じてのみレンダリングを行うため、レンダリング先が
 * DOM か Canvas（Hayate → WebGPU）かを意識しない。
 *
 * Signal・コンポーネントモデル・スケジューラは Protocol の責務外であり、
 * 各 Adapter フレームワークが持ち込む。
 */
export interface IRenderer {
  /**
   * 指定 kind の element を生成し、その {@link ElementId} を返す。
   * id は Renderer 実装が JS 側で採番する。
   */
  createElement(kind: ElementKind): ElementId;

  /**
   * レンダリングツリーのルート element を指定する。
   * Canvas Renderer では OP_SET_ROOT に対応する。
   */
  setRoot(id: ElementId): void;

  /** child を parent の末尾に追加する。 */
  appendChild(parent: ElementId, child: ElementId): void;

  /** child を parent 内の before の直前に挿入する。 */
  insertBefore(parent: ElementId, child: ElementId, before: ElementId): void;

  /** child を parent から取り除く。 */
  removeChild(parent: ElementId, child: ElementId): void;

  /**
   * element にスタイルパッチを適用する。差分のみを渡す
   * （{@link StylePatch} のセマンティクスに従う）。
   */
  setStyle(id: ElementId, style: StylePatch): void;

  /** `text` element のテキスト内容を設定する。 */
  setText(id: ElementId, text: string): void;

  /**
   * element に Interaction Event ハンドラを登録し、購読解除関数を返す。
   */
  addEventListener(
    id: ElementId,
    event: EventKind,
    handler: EventHandler,
  ): Unsubscribe;

  /**
   * レンダリングサーフェスのサイズを更新する。
   *
   * DOM Renderer では CSS が自動追従するため no-op。
   * Canvas Renderer では canvas 要素のピクセルサイズを更新し再描画を促す。
   * `renderTsubame` が ResizeObserver / window.resize から呼び出す。
   */
  resize(width: number, height: number): void;
}
