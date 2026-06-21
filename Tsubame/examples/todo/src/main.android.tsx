// グローバル shim を最初に確立する（他の import より前, ADR-0112）。
import './android-prelude';

import {
  createAndroidCanvasRenderer,
  type RawHayate,
} from '@tsubame/renderer-canvas/android';
import { renderTsubame } from '@tsubame/solid';
import { TodoApp } from './App';
import type { DetectModeResult } from './detect-mode';

/**
 * Android 用エントリ（ADR-0112）。ブラウザ用 `main.tsx` の置き換え。
 *
 * ネイティブ Hayate ホスト（JSI HostObject 等）が `globalThis.__hayateHost` として
 * 注入した {@link RawHayate} を `CanvasRenderer` に結線し、同じ `TodoApp` を
 * Canvas Mode でマウントする。フレーム駆動とリサイズはネイティブ vsync ループが
 * `globalThis.__tsubame` 経由で行う。
 */
declare global {
  // eslint-disable-next-line no-var
  var __hayateHost: RawHayate | undefined;
  // eslint-disable-next-line no-var
  var __tsubame:
    | {
        pumpFrame(timestampMs: number): void;
        resize(width: number, height: number, scale?: number): void;
        stop(): void;
      }
    | undefined;
}

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

const handle = createAndroidCanvasRenderer(raw);
renderTsubame(() => <TodoApp detected={detected} />, handle.renderer);

// ネイティブ vsync ループ用に公開する。
globalThis.__tsubame = {
  pumpFrame: (timestampMs: number) => handle.pumpFrame(timestampMs),
  resize: (width: number, height: number, scale?: number) =>
    handle.resize(width, height, scale),
  stop: () => handle.stop(),
};
