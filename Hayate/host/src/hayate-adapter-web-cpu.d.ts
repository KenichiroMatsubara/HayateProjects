// `@hayate/adapter-web-cpu`（tiny-skia CPU バックエンド）は `@hayate/adapter-web`
// と同型 package を別スコープ名にしたもの。型は同一なので re-export する。
declare module '@hayate/adapter-web-cpu' {
  export { HayateElementRenderer, HayateElementHtmlRenderer } from '@hayate/adapter-web';
  export default function init(): Promise<void>;
}
