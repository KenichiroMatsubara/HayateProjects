import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import type { RawHayate } from '../hayate.js';

export interface WasmHayateFixture {
  readonly raw: RawHayate;
  readonly canvas: HTMLCanvasElement;
  dispose(): void;
}

const wasmPath = join(
  dirname(fileURLToPath(import.meta.url)),
  '../../../../../Hayate/wasm-pkgs/pkg-null/hayate_adapter_web_bg.wasm',
);

let wasmReady = false;

/** C3 結合テスト用に null バックエンドの WASM ビルドを読み込む（ADR-0055）。 */
export async function createNullHayate(
  width = 320,
  height = 240,
): Promise<WasmHayateFixture> {
  const mod = await import('hayate-adapter-web-null');
  if (!wasmReady) {
    mod.initSync({ module: readFileSync(wasmPath) });
    wasmReady = true;
  }

  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  document.body.appendChild(canvas);

  const raw = (await mod.HayateElementRenderer.init(
    canvas,
  )) as unknown as RawHayate;

  // init はビューポートを canvas.getBoundingClientRect() から導くが、テスト DOM
  // (jsdom/happy-dom) はレイアウトを行わず 0×0 を返す。ビューポートが 0 のままだと
  // width:100%/height:100% がゼロ幾何に潰れ、ヒットテストや hover が発火しない。
  // 既知のサイズから明示的に設定する。
  (raw as unknown as { set_viewport(w: number, h: number): void }).set_viewport(
    width,
    height,
  );

  return {
    raw,
    canvas,
    dispose() {
      (raw as { free?: () => void }).free?.();
      canvas.remove();
    },
  };
}
