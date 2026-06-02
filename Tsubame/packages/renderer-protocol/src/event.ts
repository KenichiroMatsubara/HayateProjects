import type { ElementId } from './element.js';

/**
 * 要素単位の Interaction Event の種別（MVP）。
 *
 * Canvas Renderer 使用時は Hayate の `poll_events()` から受け取り、
 * DOM Renderer 使用時はネイティブ DOM イベントから橋渡しする。
 * インタラクション状態に応じたスタイル切り替え（:hover 相当）は
 * 各 Adapter フレームワークの reactivity の責務であり、Protocol は扱わない。
 *
 * MVP 後に追加予定: keydown / keyup / scroll / active-start / active-end。
 */
export type EventKind =
  | 'click'
  | 'input'
  | 'change'
  | 'keydown'
  | 'hover-enter'
  | 'hover-leave'
  | 'focus'
  | 'blur';

/**
 * ハンドラに渡される Interaction Event。Renderer 実装の差異
 * （DOM / Canvas）を吸収した最小限のペイロード。
 */
export interface InteractionEvent {
  kind: EventKind;
  /** イベントが発生した element。 */
  target: ElementId;
  /** text-input など入力系イベントで使う現在値。 */
  value?: string;
  /** keydown などキーボード系イベントで使うキー名。 */
  key?: string;
}

export type EventHandler = (event: InteractionEvent) => void;

/**
 * `addEventListener` の戻り値。呼び出すと当該ハンドラの購読を解除する。
 */
export type Unsubscribe = () => void;
