import { WasmWorkerEngine } from './worker-engine.js';
import {
  WorkerEngineDispatcher,
  type MainToWorker,
  type WorkerToMain,
} from './worker-host.js';

const scope = globalThis as unknown as {
  postMessage(message: WorkerToMain): void;
  onmessage: ((event: MessageEvent<MainToWorker>) => void) | null;
};
const dispatcher = new WorkerEngineDispatcher(
  new WasmWorkerEngine(),
  (message: WorkerToMain) => scope.postMessage(message),
);

// Preserve transport arrival order across the asynchronous WASM/renderer boot.
let dispatch = Promise.resolve();
scope.onmessage = (event: MessageEvent<MainToWorker>) => {
  dispatch = dispatch.then(() => dispatcher.handle(event.data));
};
