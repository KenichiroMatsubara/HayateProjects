export { CanvasRenderer } from './canvas-renderer.js';
export type { CanvasRendererOptions } from './canvas-renderer.js';
export type { RawHayate } from './hayate.js';
export { parseColor } from './hayate.js';
// host bootstrap（surface 取得・WASM ロード・WebGPU プローブ・backend 選択・clock 源・
// native pump）はこのパッケージに無い。Hayate 側（`@hayate/host`）または App（合成ルート）
// が持ち、host から得た `raw`(+clock) を `new CanvasRenderer({ raw, requestFrame,
// cancelFrame })` に渡す（#477, CONTEXT-MAP の依存境界）。
export {
  encodeStylePatch,
  unsetKindsOf,
  TAG,
  UNSET_KIND,
} from '@tsubame/protocol-generated/codec';
export { OP, ELEMENT_KIND } from '@tsubame/protocol-generated/protocol';
// wire の protocol version（バンドル encoder ↔ ホスト decoder の整合トークン）。App Bundle は内包する
// renderer-canvas のこの版数を Miharashi 起動時の突き合わせ用に埋める（#530 / CONTEXT「Protocol Version」）。
export { PROTOCOL_VERSION } from '@tsubame/protocol-generated/protocol';
