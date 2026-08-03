// 合成ルート（ADR-0012）。target 選択は Host、FW 固有 mount は TsubameMount に局在し、
// runTsubameApp はそのどちらも知らない（@torimi/tsubame-renderer-protocol だけに依存）。
export { runTsubameApp } from './run.js';
export type {
  AppHandle,
  AppStartResult,
  Dispose,
  Host,
  HostSession,
  TsubameMount,
} from './host.js';
