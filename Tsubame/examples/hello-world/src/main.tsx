import type { IRenderer } from '@tsubame/renderer-protocol';
import { DomRenderer } from '@tsubame/renderer-dom';
import { renderTsubame } from '@tsubame/solid';
import { TodoApp, type Mode, type ModeSource } from './App';

function detectMode(): { mode: Mode; source: ModeSource } {
  const q = new URLSearchParams(window.location.search).get('mode');
  if (q === 'dom') return { mode: 'DOM', source: 'query' };
  if (q === 'canvas') return { mode: 'Canvas', source: 'query' };
  // 自動判定は Hayate の Canvas Mode / HTML Mode の精神を踏襲し、
  // WebGPU + EditContext API が両方使える環境のみ Canvas を選ぶ。
  const hasWebGPU = 'gpu' in navigator;
  const hasEditContext = 'EditContext' in window;
  return {
    mode: hasWebGPU && hasEditContext ? 'Canvas' : 'DOM',
    source: 'auto',
  };
}

const { mode, source } = detectMode();

const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;
const srcLabel = document.getElementById('src-label')!;
const linkDom = document.getElementById('link-dom')!;
const linkCanvas = document.getElementById('link-canvas')!;
const linkAuto = document.getElementById('link-auto')!;

srcLabel.textContent = source === 'query' ? '?mode=' + mode.toLowerCase() : 'auto-detected';
(source === 'query'
  ? (mode === 'DOM' ? linkDom : linkCanvas)
  : linkAuto
).classList.add('active');

let renderer: IRenderer;
if (mode === 'DOM') {
  dom.hidden = false;
  renderer = new DomRenderer({ container: dom });
  renderTsubame(() => <TodoApp mode={mode} source={source} />, renderer, { element: dom });
} else {
  const [{ CanvasRenderer }, { MockHayate }] = await Promise.all([
    import('@tsubame/renderer-canvas'),
    import('./mock-hayate'),
  ]);
  const w = window.innerWidth;
  const h = window.innerHeight;
  canvas.hidden = false;
  canvas.width = w;
  canvas.height = h;
  canvas.style.width = `${w}px`;
  canvas.style.height = `${h}px`;
  renderer = new CanvasRenderer(new MockHayate(canvas));
  renderTsubame(() => <TodoApp mode={mode} source={source} />, renderer, { element: canvas });
}
