// グローバル shim を最初に確立する（他の import より前, ADR-0112）。
import './android-prelude';

import { createHayateNativeHost, type RawHayate } from '@hayate/host/native';
import { renderTsubame } from '@tsubame/solid';
import { HayateRenderer, PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { runTsubameApp, type DetectModeResult, type Host } from '@tsubame/app';
import { TodoApp } from './App';

/**
 * Android 用エントリ（ADR-0112）。ブラウザ用 `main.tsx` の置き換えで、同じ薄い対称
 * 合成ルート「host→raw(+clock)→HayateRenderer→mount」に縮約される（#477）。
 *
 * ネイティブ Hayate ホスト（JSI HostObject 等）が `globalThis.__hayateHost` として
 * 注入した {@link RawHayate} を `@hayate/host/native` の host へ渡す。host は WASM を
 * ロードせず（Hayate はネイティブ cdylib として既に存在）、frame-clock をネイティブ
 * vsync が 1 フレームずつ駆動する pump として供給する。viewport 追従（resize）は
 * native ループが `tree.set_viewport` を直接駆動するため JS 経路には無い（ADR-0080 を
 * native へ延長, issue #475）。
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
// （#530 / #533）。global 名は `@torimi/protocol-handshake` の TORIMI_PROTOCOL_VERSION_GLOBAL
// （'__torimiProtocolVersion'）と一致させる wire 契約。native ホスト（hayate-adapter-android の
// app_tsubame）は eval 後にこれを読み、自身に焼き込んだ decoder 版数（hayate_core::wire::
// PROTOCOL_VERSION）と突き合わせて一致時のみ mount する。Web の `main.torimi.tsx` と対称。
globalThis.__torimiProtocolVersion = PROTOCOL_VERSION;

const raw = globalThis.__hayateHost;
if (raw === undefined) {
  throw new Error(
    'Android: globalThis.__hayateHost (native RawHayate) が注入されていません',
  );
}

// Android はネイティブ Vello/Vulkan の Canvas Mode 固定。ブラウザのような
// DOM/WebGPU 検出は行わない（ADR-0112）。
const detected: DetectModeResult = {
  mode: 'Canvas',
  backend: 'vello',
  source: 'query',
  renderer: 'vello',
};

// native host（注入 raw + vsync pump）を Host adapter に包む。target は Hayate 固定で、
// createRenderer は host-blind HayateRenderer を構築するだけ（ADR-0012）。web の bundle 経路
// （main.torimi.tsx）と同型 — 押し込まれた host を `Host` に包んで `runTsubameApp` を呼ぶ。
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

const dispose = runTsubameApp(host, (renderer) =>
  renderTsubame(() => <TodoApp detected={detected} />, renderer),
);

// ネイティブ vsync ループ用に公開する。pump は保持中のフレームを 1 つ進め、
// `HayateRenderer` が次フレームを再登録する。stop は frame-clock を解除する。
globalThis.__tsubame = {
  pumpFrame: (timestampMs: number) => nativeHost.pumpFrame(timestampMs),
  stop: dispose,
};
