import { DomRenderer } from '@tsubame/renderer-dom';
import { renderTsubame } from '@tsubame/solid';
import { TodoApp } from './App';
import { mountCanvasApp } from './compose';
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

if (detected.mode === 'DOM') {
  // DOM Renderer 経路は無変更：Hayate を迂回し、native IME と CSS リフローに委ねる。
  // viewport 追従はブラウザの CSS / `@media` が担い、Tsubame は resize を配線しない
  // （ADR-0080, issue #475）。
  dom.hidden = false;
  const renderer = new DomRenderer({ container: dom });
  renderTsubame(() => <TodoApp detected={detected} />, renderer);
} else {
  // Canvas 経路：host bootstrap は Hayate 側（`@hayate/host`）が持つ。host が WebGPU を
  // プローブし backend を選び WASM をロードして surface 上に raw を確立し、frame-clock
  // （rAF）も供給する。App は host から raw(+clock) を得て host-blind HayateRenderer に
  // 結線し mount するだけ — native（`main.android.tsx`）と対称な薄い合成ルート（#477）。
  const { createHayateWebHost } = await import('@hayate/host');
  canvas.hidden = false;
  // Dev-only: pick up a hand-edited `tuning.json` from the served root so taste
  // constants (scroll physics, scrollbar chrome, …) can be calibrated by editing the
  // file and pressing F5 — no WASM rebuild (#353 family). Missing file (404) or parse
  // failure simply leaves the compiled defaults in place.
  const tuning = await fetch(new URL('tuning.jsonc', document.baseURI).href)
    .then((r) => (r.ok ? r.text() : undefined))
    .catch(() => undefined);
  const host = await createHayateWebHost(canvas, { backend: detected.backend, tuning });
  // hayate-adapter-web owns viewport sizing / pointer / IME — its self-wired
  // ResizeObserver reads the live devicePixelRatio each fire (ADR-0080 / ADR-0069).
  mountCanvasApp(host, (renderer) =>
    renderTsubame(() => <TodoApp detected={detected} />, renderer),
  );
}
