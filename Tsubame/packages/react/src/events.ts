// イベント語彙の正本は `@torimi/tsubame-renderer-protocol` にある（ADR-0010）。
// `<view onClick>` 等の意味が Tsubame Adapter 間でドリフトしないよう、
// tsubame-react は protocol の語彙を再 export するだけに留める。
export { EVENT_PROP, REJECTED_EVENT_PROPS } from '@torimi/tsubame-renderer-protocol';
