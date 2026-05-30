import type { IRenderer } from '@tsubame/renderer-protocol';
import { DomRenderer } from '@tsubame/renderer-dom';
import { CanvasRenderer } from '@tsubame/renderer-canvas';
import { renderTsubame } from '@tsubame/solid';
import { App } from './App';
import { MockHayate } from './mock-hayate';

type Mode = 'DOM' | 'Canvas';

const domHost = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;
const swapButton = document.getElementById('swap') as HTMLButtonElement;
const modeLabel = document.getElementById('mode') as HTMLSpanElement;

// 現在マウント中のリソース。切替時に確実に破棄する。
let dispose: (() => void) | null = null;
let canvasRenderer: CanvasRenderer | null = null;
let mockHayate: MockHayate | null = null;

function teardown(): void {
  dispose?.();
  dispose = null;
  canvasRenderer?.stop();
  canvasRenderer = null;
  mockHayate?.dispose();
  mockHayate = null;
}

function mount(mode: Mode): void {
  teardown();
  modeLabel.textContent = mode;

  let renderer: IRenderer;
  if (mode === 'DOM') {
    domHost.hidden = false;
    canvas.hidden = true;
    renderer = new DomRenderer({ container: domHost });
  } else {
    domHost.hidden = true;
    canvas.hidden = false;
    mockHayate = new MockHayate(canvas);
    canvasRenderer = new CanvasRenderer(mockHayate);
    renderer = canvasRenderer;
  }

  // 同一の App を Renderer だけ差し替えてマウントする。
  dispose = renderTsubame(App, renderer);
}

let current: Mode = 'DOM';
mount(current);

// onclick 代入は冪等（HMR 再実行でもリスナーが重複しない）。
swapButton.onclick = () => {
  current = current === 'DOM' ? 'Canvas' : 'DOM';
  mount(current);
};
