// `@torimi/hayate-adapter-web-cpu`（tiny-skia CPU バックエンド）は `@torimi/hayate-adapter-web`
// と同型 package を別スコープ名にしたもの。型は同一なので re-export する。
declare module '@torimi/hayate-adapter-web-cpu' {
  export { HayateElementRenderer, HayateElementHtmlRenderer } from '@torimi/hayate-adapter-web';
  export default function init(): Promise<void>;
}
