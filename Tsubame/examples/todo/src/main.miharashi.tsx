import { renderTsubame } from '@tsubame/solid';
import { PROTOCOL_VERSION } from '@tsubame/renderer-canvas';
import type { WebHost } from '@hayate/host';
import { TodoApp } from './App';
import { mountCanvasApp } from './compose';
import type { DetectModeResult } from './detect-mode';

/**
 * Miharashi App Bundle エントリ（ADR-0001 のスライス #1）。Miharashi Web ホストが
 * dev-server から fetch → eval する単一 IIFE バンドルの入口。
 *
 * ホストは host bootstrap（`createHayateWebHost` の `raw` + frame-clock）だけを提供し、
 * フレームワークも `@tsubame/renderer-canvas` も持たない。バンドル側（ここ）が solid と
 * `CanvasRenderer` を持ち込み、ホストから渡された host bootstrap で TodoApp を mount する。
 *
 * native の `main.android.tsx`（`globalThis.__tsubame` を露出）と対称に、ここでは
 * `globalThis.__miharashiMount`（host bootstrap → mount）を露出する受け渡しシーム。
 */

// Miharashi ホストは Canvas モード固定（host が WebGPU を auto 判定して backend を選ぶ）。
// backend はホスト側が決めるためバンドルは知らない（badge は 'Canvas' 表示）。
const detected: DetectModeResult = {
  mode: 'Canvas',
  source: 'auto',
  renderer: 'auto',
};

declare global {
  // eslint-disable-next-line no-var
  var __miharashiMount: ((host: WebHost) => void) | undefined;
  // eslint-disable-next-line no-var
  var __miharashiProtocolVersion: number | undefined;
}

// 内包する `@tsubame/renderer-canvas` の wire 定数バージョンを protocol version として埋める
// （#530）。global 名は `@miharashi/protocol-handshake` の MIHARASHI_PROTOCOL_VERSION_GLOBAL
// （'__miharashiProtocolVersion'）と一致させる wire 契約。ホストは eval 後にこれを読み、自身の
// decoder 版数と突き合わせて一致時のみ mount する。
globalThis.__miharashiProtocolVersion = PROTOCOL_VERSION;

// `@miharashi/host-web` の MIHARASHI_MOUNT_GLOBAL（'__miharashiMount'）と一致させる
// wire 契約。ホストは eval 後にこの global を読み、host bootstrap を渡して呼ぶ。
globalThis.__miharashiMount = (host: WebHost) => {
  mountCanvasApp(host, (renderer) =>
    renderTsubame(() => <TodoApp detected={detected} />, renderer),
  );
};
