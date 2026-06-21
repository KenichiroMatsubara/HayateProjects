import type { ElementId } from './element.js';

/**
 * 要素単位の Interaction Event の種別（MVP）。
 *
 * Canvas Renderer 使用時は Hayate の `poll_events()` delivery から受け取り、
 * DOM Renderer 使用時はネイティブ DOM イベントから橋渡しする。
 * `:hover` / `:active` / `:focus` スタイルは Hayate Render Layer（ADR-0056）が
 * 解決する。Tsubame Adapter は hover イベント購読を拒否する（ADR-0059）。
 */
export type { EventKind } from './generated/event-kind.js';
import type { EventKind } from './generated/event-kind.js';

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
