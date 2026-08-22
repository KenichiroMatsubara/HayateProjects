import { renderTsubame } from '@torimi/tsubame-react';
import { createBrowserHost } from '@torimi/tsubame-browser-host';
import { runTsubameApp, type AppHandle } from '@torimi/tsubame-app';
import { App } from './App';

// 将棋デモも solid / react デモと同じ合成ルートに乗る。target（DOM / Hayate）の選択は Host に局在し、
// FW 固有なのは mount の 1 行（`renderTsubame(<App/>, renderer)`）だけ（ADR-0012）。
// `vite dev` でも EditContext があれば Hayate に描画し、Auto の backend 順序は Host に委ねる。
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

const host = createBrowserHost({ dom, canvas });

const runningApp = runTsubameApp(host, (renderer) =>
  renderTsubame(<App />, renderer),
);

let disposed = false;
const dispose = (): void => {
  if (disposed) return;
  disposed = true;
  window.removeEventListener('pagehide', dispose);
  runningApp.dispose();
};

/** Observable demo lifetime for integration tests and embedding shells. */
export const appHandle: AppHandle = {
  settled: runningApp.settled,
  dispose,
};

window.addEventListener('pagehide', dispose, { once: true });
