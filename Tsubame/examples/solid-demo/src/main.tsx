import { createBrowserHost } from '@torimi/tsubame-browser-host';
import { renderTsubame } from '@torimi/tsubame-solid';
import { runTsubameApp, type AppHandle } from '@torimi/tsubame-app';
import { TodoApp } from './App';
import { WorkerScrollWorkload } from './WorkerScrollWorkload';

const useWorkerScrollWorkload =
  new URLSearchParams(window.location.search).get('workload') === 'worker-scroll';
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// Target selection and browser resource lifetime are the Browser Host's deep interface. This
// framework entry contributes only its two surfaces and the Solid mount closure (ADR-0015).
const host = createBrowserHost({
  dom,
  canvas,
  tuning: { kind: 'optional-url', url: '/tuning.jsonc' },
});

const runningApp = runTsubameApp(host, (renderer) =>
  renderTsubame(
    () => (useWorkerScrollWorkload ? <WorkerScrollWorkload /> : <TodoApp />),
    renderer,
  ),
);

// This demo explicitly publishes the Browser Host's narrow inspection for Playwright. The
// package itself never writes globals, and the reserved native RawHayate name stays untouched.
const inspection = host.inspection();
const browserDebug = window as unknown as {
  __tsubameBrowserHostInspection?: typeof inspection;
};
browserDebug.__tsubameBrowserHostInspection = inspection;

let disposed = false;
const dispose = (): void => {
  if (disposed) return;
  disposed = true;
  window.removeEventListener('pagehide', dispose);
  if (browserDebug.__tsubameBrowserHostInspection === inspection) {
    delete browserDebug.__tsubameBrowserHostInspection;
  }
  runningApp.dispose();
};

/** Observable demo lifetime for integration tests and embedding shells. */
export const appHandle: AppHandle = {
  settled: runningApp.settled,
  dispose,
};

window.addEventListener('pagehide', dispose, { once: true });
const hot = (
  import.meta as ImportMeta & {
    readonly hot?: { dispose(callback: () => void): void };
  }
).hot;
hot?.dispose(dispose);
