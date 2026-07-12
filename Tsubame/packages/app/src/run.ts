import type { IRenderer } from '@torimi/tsubame-renderer-protocol';
import type { Dispose, Host, TsubameMount } from './host.js';

function isPromise<T>(value: T | Promise<T>): value is Promise<T> {
  return typeof (value as { then?: unknown }).then === 'function';
}

/**
 * Tsubame アプリの合成ルート（ADR-0012）。
 *
 * `host` から `IRenderer` を得て `mount` に渡すだけの deep module。target 選択（DOM / Hayate・
 * web / native / bundle）は `host` に、FW 固有の mount は `mount` に局在し、ここは両者を
 * 知らない（`@torimi/tsubame-renderer-protocol` だけに依存し、具体 renderer も Hayate ランタイムも
 * import しない）。`Host.createRenderer()` が非同期（WASM ロード）でも、返す {@link Dispose} は
 * 同期で受け取れる — resolve 前に dispose されたら mount しない。
 *
 * @returns mount のツリー破棄と `host.stop()` を合成した dispose。
 */
export function runTsubameApp(host: Host, mount: TsubameMount): Dispose {
  let disposed = false;
  let mountDispose: Dispose | void;

  const onRenderer = (renderer: IRenderer): void => {
    if (disposed) return;
    mountDispose = mount(renderer);
  };

  const created = host.createRenderer();
  if (isPromise(created)) {
    created.then(onRenderer).catch((error: unknown) => {
      // host 構築失敗は握り潰さず可視化する（テストは境界を読む）。
      // eslint-disable-next-line no-console
      console.error(error);
    });
  } else {
    onRenderer(created);
  }

  return () => {
    if (disposed) return;
    disposed = true;
    if (typeof mountDispose === 'function') mountDispose();
    host.stop?.();
  };
}
