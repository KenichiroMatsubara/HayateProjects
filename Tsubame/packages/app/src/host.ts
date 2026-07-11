import type { IRenderer } from '@torimi/tsubame-renderer-protocol';

/** ツリー破棄や frame-clock 解除を行う後始末関数。 */
export type Dispose = () => void;

/**
 * 合成ルートに renderer を供給する port（ADR-0012）。
 *
 * 「DOM か Hayate か」「web か native か bundle か」の分岐は **この実装の内側に局在する** —
 * `createRenderer()` は `DomRenderer` か `HayateRenderer` を構築して `IRenderer` として返す。
 * 合成ルート（{@link runTsubameApp}）は具体 renderer 名も platform も知らない。platform 増殖
 * （web-vello / web-tinyskia / Android / 将来 iOS・Desktop）はすべて Host を 1 つ足す仕事に縮む。
 *
 * - **DOM Host**: `createRenderer` は `new DomRenderer({ container })` を即時に返す。`stop` は不要。
 * - **Hayate Host（web）**: `createRenderer` は `@torimi/hayate-host` の `createHayateWebHost` を await し
 *   `new HayateRenderer({ raw, requestFrame, cancelFrame })` を `start()` して返す。`stop` で
 *   frame-clock / WASM を畳む。
 * - **Hayate Host（native / bundle）**: 注入された `raw`(+clock) を包んで同形に返す。
 */
export interface Host {
  /** renderer を構築して返す。WASM ロード等のため Promise でもよい。 */
  createRenderer(): IRenderer | Promise<IRenderer>;
  /** frame-clock / WASM teardown（DOM Host では未定義でよい）。 */
  stop?(): void;
}

/**
 * 合成ルートにおける唯一の FW 固有 seam（ADR-0012）。
 *
 * 各 Tsubame Adapter（solid / react / vue）が、自分の reactivity でコンポーネントツリーを
 * `IRenderer` に mount する 1 関数として供給する。solid は `() => JSX`、react は `ReactNode`
 * という `renderTsubame` の呼び形の差をこの内側に閉じ込め、合成ルートには一様な形で現れる。
 * 戻り値の {@link Dispose} はツリー破棄に使う（`renderTsubame` の dispose を素通しでよい）。
 */
export type TsubameMount = (renderer: IRenderer) => Dispose | void;
