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
  // viewport 追従はブラウザの CSS リフロー / `@media` が担う。Tsubame は resize を
  // 配線しない（ADR-0080, issue #475）。
  renderTsubame(() => <TodoApp detected={detected} />, renderer);
} else {
  const { initCanvasRenderer } = await import('@tsubame/renderer-canvas');
  canvas.hidden = false;
  // Dev-only: pick up a hand-edited `tuning.json` from the served root so
  // taste constants (scroll physics, scrollbar chrome, …) can be calibrated by
  // editing the file and pressing F5 — no WASM rebuild (#353 family). Missing
  // file (404) or parse failure simply leaves the compiled defaults in place.
  const tuning = await fetch(new URL('tuning.jsonc', document.baseURI).href)
    .then((r) => (r.ok ? r.text() : undefined))
    .catch(() => undefined);
  renderer = await initCanvasRenderer(canvas, { backend: detected.backend, tuning });
  // hayate-adapter-web owns viewport sizing — its self-wired ResizeObserver reads
  // the live devicePixelRatio each fire (ADR-0080, superseding ADR-0007's
  // host-owned observer). No element option needed.
  renderTsubame(() => <TodoApp detected={detected} />, renderer);
}
