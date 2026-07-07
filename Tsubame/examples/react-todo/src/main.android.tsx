// グローバル shim を最初に確立する（他の import より前, ADR-0112）。
import './android-prelude';

import { createHayateNativeHost, type RawHayate } from '@hayate/host/native';
import { renderTsubame } from '@tsubame/react';
import { HayateRenderer, PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { runTsubameApp, type Host } from '@tsubame/app';
import { App } from './App';

/**
 * react の Android 用エントリ（#739）。solid 版（`examples/todo/src/main.android.tsx`）と
 * 同型の薄い合成ルート「host→raw(+clock)→HayateRenderer→mount」で、FW だけが react に
 * 替わる。露出する wire シーム（`__torimiProtocolVersion` / `__tsubame`）は solid と
 * 同一 — Torimi ホストは中身の FW を解さないので、この対称性が「Viewer 一本で全 JS FW
 * が動く」の実体（ADR-0001 / ADR-0003）。
 *
 * ネイティブ Hayate ホスト（JSI HostObject 等）が `globalThis.__hayateHost` として注入した
 * {@link RawHayate} を `@hayate/host/native` の host へ渡す。frame-clock はネイティブ vsync が
 * `__tsubame.pumpFrame` で 1 フレームずつ駆動する。viewport 追従（resize）は native ループが
 * `tree.set_viewport` を直接駆動するため JS 経路には無い（issue #475）。
 */
declare global {
  // eslint-disable-next-line no-var
  var __hayateHost: RawHayate | undefined;
  // eslint-disable-next-line no-var
  var __tsubame:
    | {
        pumpFrame(timestampMs: number): void;
        stop(): void;
      }
    | undefined;
  // eslint-disable-next-line no-var
  var __torimiProtocolVersion: number | undefined;
}

// 内包する `@tsubame/renderer-hayate` の wire 定数バージョンを protocol version として埋める
// （#530 / #533）。native ホスト（hayate-adapter-android の app_tsubame）は eval 後にこれを
// 読み、自身に焼き込んだ decoder 版数と突き合わせて一致時のみ mount する（solid 版と対称）。
globalThis.__torimiProtocolVersion = PROTOCOL_VERSION;

const raw = globalThis.__hayateHost;
if (raw === undefined) {
  throw new Error(
    'Android: globalThis.__hayateHost (native RawHayate) が注入されていません',
  );
}

// native host（注入 raw + vsync pump）を Host adapter に包む。solid 版と同型で、
// createRenderer は host-blind HayateRenderer を構築するだけ（ADR-0012）。
const nativeHost = createHayateNativeHost(raw);
let hayateRenderer: HayateRenderer | undefined;
const host: Host = {
  createRenderer() {
    hayateRenderer = new HayateRenderer({
      raw: nativeHost.raw,
      requestFrame: nativeHost.requestFrame,
      cancelFrame: nativeHost.cancelFrame,
    });
    hayateRenderer.start();
    return hayateRenderer;
  },
  stop: () => hayateRenderer?.stop(),
};

const dispose = runTsubameApp(host, (renderer) => renderTsubame(<App />, renderer));

// ネイティブ vsync ループ用に公開する。pump は保持中のフレームを 1 つ進め、
// `HayateRenderer` が次フレームを再登録する。stop は frame-clock を解除する。
globalThis.__tsubame = {
  pumpFrame: (timestampMs: number) => nativeHost.pumpFrame(timestampMs),
  stop: dispose,
};
