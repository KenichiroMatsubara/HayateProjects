// 合成ルート（ADR-0012）。target 選択は Host、FW 固有 mount は TsubameMount に局在し、
// runTsubameApp はそのどちらも知らない（@torimi/tsubame-renderer-protocol だけに依存）。
export { runTsubameApp } from './run.js';
export type { Host, TsubameMount, Dispose } from './host.js';

// web 専用の DOM 退避判定。Canvas backend の語彙・選択順・query 解釈は Hayate Host が持つ。
export { shouldUseDomRenderer } from './web-target.js';
