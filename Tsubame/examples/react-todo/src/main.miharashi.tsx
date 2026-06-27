import type { WebHost } from '@hayate/host';
import { renderTsubame } from '@tsubame/react';
import { HayateRenderer, PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { runTsubameApp, type Host } from '@tsubame/app';
import { App } from './App';

/**
 * Miharashi react App Bundle エントリ（#531：FW 非依存の実証）。Miharashi Web ホストが
 * dev-server から fetch → eval する単一 IIFE バンドルの入口。
 *
 * ホストは host bootstrap（`createHayateWebHost` の `raw` + frame-clock）だけを提供し、
 * フレームワークも `@tsubame/renderer-hayate` も持たない。バンドル側（ここ）が **react** と
 * `HayateRenderer` を持ち込み、ホストから渡された host bootstrap で App を mount する。
 *
 * solid の `examples/todo/src/main.miharashi.tsx` と対称：FW を差し替えても露出する wire
 * シーム（`__miharashiMount` / `__miharashiProtocolVersion`）は同一なので、同じホストが両方を
 * 描画できる（ADR-0001 / CONTEXT.md「Host」）。
 */

declare global {
  // eslint-disable-next-line no-var
  var __miharashiMount: ((host: WebHost) => void) | undefined;
  // eslint-disable-next-line no-var
  var __miharashiProtocolVersion: number | undefined;
}

// 内包する `@tsubame/renderer-hayate` の wire 定数バージョンを protocol version として埋める
// （#530）。ホストは eval 後にこれを読み、自身の decoder 版数と突き合わせて一致時のみ mount する。
globalThis.__miharashiProtocolVersion = PROTOCOL_VERSION;

// `@miharashi/host-web` の MIHARASHI_MOUNT_GLOBAL（'__miharashiMount'）と一致させる wire 契約。
// ホストは eval 後にこの global を読み、host bootstrap を渡して呼ぶ。バンドルはここで react と
// HayateRenderer を結線する（host は両者を知らない）。
globalThis.__miharashiMount = (webHost: WebHost) => {
  // 押し込まれた host(raw+clock) を Host adapter に包み、合成ルートへ渡す。solid の
  // main.miharashi.tsx と同型 — 露出する wire シームは FW 非依存（ADR-0012 / ADR-0001）。
  const host: Host = {
    createRenderer() {
      const renderer = new HayateRenderer({
        raw: webHost.raw,
        requestFrame: webHost.requestFrame,
        cancelFrame: webHost.cancelFrame,
      });
      renderer.start();
      return renderer;
    },
  };
  runTsubameApp(host, (renderer) => renderTsubame(<App />, renderer));
};
