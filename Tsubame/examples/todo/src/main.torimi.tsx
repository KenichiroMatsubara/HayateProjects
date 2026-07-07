import { renderTsubame } from '@tsubame/solid';
import { HayateRenderer, PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { runTsubameApp, type DetectModeResult, type Host } from '@tsubame/app';
import type { WebHost } from '@hayate/host';
import { TodoApp } from './App';

/**
 * Torimi App Bundle エントリ（ADR-0001 のスライス #1）。Torimi Web ホストが
 * dev-server から fetch → eval する単一 IIFE バンドルの入口。
 *
 * ホストは host bootstrap（`createHayateWebHost` の `raw` + frame-clock）だけを提供し、
 * フレームワークも `@tsubame/renderer-hayate` も持たない。バンドル側（ここ）が solid と
 * `HayateRenderer` を持ち込み、ホストから渡された host bootstrap で TodoApp を mount する。
 *
 * native の `main.android.tsx`（`globalThis.__tsubame` を露出）と対称に、ここでは
 * `globalThis.__torimiMount`（host bootstrap → mount）を露出する受け渡しシーム。
 */

// Torimi ホストは Canvas モード固定（host が WebGPU を auto 判定して backend を選ぶ）。
// backend はホスト側が決めるためバンドルは知らない（badge は 'Canvas' 表示）。
const detected: DetectModeResult = {
  mode: 'Canvas',
  source: 'auto',
  renderer: 'auto',
};

declare global {
  // eslint-disable-next-line no-var
  var __torimiMount: ((host: WebHost) => void) | undefined;
  // eslint-disable-next-line no-var
  var __torimiProtocolVersion: number | undefined;
}

// 内包する `@tsubame/renderer-hayate` の wire 定数バージョンを protocol version として埋める
// （#530）。global 名は `@torimi/protocol-handshake` の TORIMI_PROTOCOL_VERSION_GLOBAL
// （'__torimiProtocolVersion'）と一致させる wire 契約。ホストは eval 後にこれを読み、自身の
// decoder 版数と突き合わせて一致時のみ mount する。
globalThis.__torimiProtocolVersion = PROTOCOL_VERSION;

// `@torimi/host-web` の TORIMI_MOUNT_GLOBAL（'__torimiMount'）と一致させる
// wire 契約。ホストは eval 後にこの global を読み、host bootstrap を渡して呼ぶ。
globalThis.__torimiMount = (webHost: WebHost) => {
  // 押し込まれた host(raw+clock) を Host adapter に包み、host-blind HayateRenderer を構築する。
  // native（main.android.tsx）と同型の薄い合成（ADR-0012）。
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
  runTsubameApp(host, (renderer) =>
    renderTsubame(() => <TodoApp detected={detected} />, renderer),
  );
};
