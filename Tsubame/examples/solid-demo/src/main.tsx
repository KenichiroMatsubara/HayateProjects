import { createBrowserHost } from '@torimi/tsubame-browser-host';
import { renderTsubame } from '@torimi/tsubame-solid';
import { runTsubameApp } from '@torimi/tsubame-app';
import { TodoApp } from './App';
import { WorkerScrollWorkload } from './WorkerScrollWorkload';

const useWorkerScrollWorkload =
  new URLSearchParams(window.location.search).get('workload') === 'worker-scroll';
const dom = document.getElementById('dom-host') as HTMLDivElement;
const canvas = document.getElementById('canvas-stage') as HTMLCanvasElement;

// Target selection and browser resource lifetime are the Browser Host's deep interface. This
// framework entry contributes only its two surfaces and the Solid mount closure (ADR-0015).
const host = createBrowserHost({ dom, canvas });

runTsubameApp(host, (renderer) =>
  renderTsubame(
    () => (useWorkerScrollWorkload ? <WorkerScrollWorkload /> : <TodoApp />),
    renderer,
  ),
);
