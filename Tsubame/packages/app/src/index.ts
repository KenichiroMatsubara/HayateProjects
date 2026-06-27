// 合成ルート（ADR-0012）。target 選択は Host、FW 固有 mount は TsubameMount に局在し、
// runTsubameApp はそのどちらも知らない（@tsubame/renderer-protocol だけに依存）。
export { runTsubameApp } from './run.js';
export type { Host, TsubameMount, Dispose } from './host.js';

// web 専用の DOM/Canvas 判定（依存ゼロの純粋関数）。orchestrator は呼ばない — App の entry が
// 呼んで結果を Host 構築と mount クロージャの両方へ渡す（CONTEXT「Composition Root」）。
export { detectMode, detectModeFromSearch, parseRendererQuery } from './detect-mode.js';
export type {
  Mode,
  ModeSource,
  RendererQuery,
  CanvasBackend,
  DetectModeInput,
  DetectModeResult,
} from './detect-mode.js';
