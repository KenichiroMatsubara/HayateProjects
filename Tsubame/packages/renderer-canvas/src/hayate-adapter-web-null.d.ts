// null backend（`hayate-adapter-web-null`）は ADR-0055 の codec/golden 結合テスト専用
// fixture（devDependency）。出荷コードは host adapter を import しない（#477）ので、この
// shim は production の `hayate-adapter-web` 型に依存せず自己完結で `HayateElementRenderer`
// を宣言する。実体は wasm-bindgen 生成クラスで、`RawHayate` を構造的に充足する。
declare module 'hayate-adapter-web-null' {
  export class HayateElementRenderer {
    static init(canvas: HTMLCanvasElement): Promise<unknown>;
  }
  export function initSync(module: { module: BufferSource }): void;
  export default function init(): Promise<void>;
}
