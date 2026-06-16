import type { IRenderer } from '@tsubame/renderer-protocol';
import { DomRenderer } from '@tsubame/renderer-dom';
import { renderTsubame } from '@tsubame/solid';
import { TodoApp } from './App';
import { detectModeFromSearch, type DetectModeResult } from './detect-mode';

function detectModeFromWindow(): DetectModeResult {
  return detectModeFromSearch(window.location.search, {
    hasEditContext: 'EditContext' in window,
    hasWebGPU: 'gpu' in navigator,
  });
}

const detected = detectModeFromWindow();

const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

let renderer: IRenderer;
if (detected.mode === 'DOM') {
  dom.hidden = false;
  renderer = new DomRenderer({ container: dom });
  renderTsubame(() => <TodoApp detected={detected} />, renderer, { element: dom });
} else {
  const { initCanvasRenderer } = await import('@tsubame/renderer-canvas');
  canvas.hidden = false;
  renderer = await initCanvasRenderer(canvas, { backend: detected.backend });
  // CanvasRenderer owns viewport sizing (ADR-0007); no element option needed.
  renderTsubame(() => <TodoApp detected={detected} />, renderer);
}
