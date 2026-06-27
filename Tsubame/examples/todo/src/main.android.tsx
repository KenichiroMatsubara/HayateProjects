// グローバル shim を最初に確立する（他の import より前, ADR-0112）。
import './android-prelude';

import { createHayateNativeHost, type RawHayate } from '@hayate/host/native';
import { renderTsubame } from '@tsubame/solid';
import { PROTOCOL_VERSION } from '@tsubame/renderer-hayate';
import { TodoApp } from './App';
import { mountCanvasApp } from './compose';
import type { DetectModeResult } from './detect-mode';

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
  var __miharashiProtocolVersion: number | undefined;
}

// 内包する `@tsubame/renderer-hayate` の wire 定数バージョンを protocol version として埋める
// （#530 / #533）。global 名は `@miharashi/protocol-handshake` の MIHARASHI_PROTOCOL_VERSION_GLOBAL
// （'__miharashiProtocolVersion'）と一致させる wire 契約。native ホスト（hayate-adapter-android の
// app_tsubame）は eval 後にこれを読み、自身に焼き込んだ decoder 版数（hayate_core::wire::
// PROTOCOL_VERSION）と突き合わせて一致時のみ mount する。Web の `main.miharashi.tsx` と対称。
globalThis.__miharashiProtocolVersion = PROTOCOL_VERSION;

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

const host = createHayateNativeHost(raw);
const renderer = mountCanvasApp(host, (r) =>
  renderTsubame(() => <TodoApp detected={detected} />, r),
);

// ネイティブ vsync ループ用に公開する。pump は保持中のフレームを 1 つ進め、
// `HayateRenderer` が次フレームを再登録する。stop は frame-clock を解除する。
globalThis.__tsubame = {
  pumpFrame: (timestampMs: number) => host.pumpFrame(timestampMs),
  stop: () => renderer.stop(),
};
