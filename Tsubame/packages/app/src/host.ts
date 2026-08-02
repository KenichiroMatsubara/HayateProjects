import type { IRenderer } from '@torimi/tsubame-renderer-protocol';

/** ツリー・frame-clock・runtime 資源を破棄する後始末関数。 */
export type Dispose = () => void;

/** 一回の Host.start が生成した renderer と teardown の不可分な lifetime value。 */
export interface HostSession {
  readonly renderer: IRenderer;
  /** 必須かつ冪等。 */
  dispose(): void;
}

/** renderer と platform runtime の lifetime を開始する port（ADR-0015）。 */
export interface Host {
  start(): HostSession | Promise<HostSession>;
}

/** 合成ルートにおける唯一の framework 固有 seam。 */
export type TsubameMount = (renderer: IRenderer) => Dispose | void;

export type AppStartResult =
  | { readonly status: 'mounted' }
  | { readonly status: 'failed'; readonly error: unknown }
  | { readonly status: 'disposed' };

/** app lifetime と、reject しない開始結果。 */
export interface AppHandle {
  readonly settled: Promise<AppStartResult>;
  dispose(): void;
}
