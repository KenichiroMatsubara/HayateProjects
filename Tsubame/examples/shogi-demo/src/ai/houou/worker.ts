/// <reference lib="webworker" />

/**
 * Houou（自作 Rust エンジン）を回す Worker。
 *
 * **wasm の探索は途中で譲らない**（1 手を最後まで読み切る）。だからメインスレッドでは
 * 走らせられず、ここに隔離する。UI 側は `poll` で現在の状態を見るだけになり、
 * `engine.ts` が定めた「非同期エンジンは何もせず現在の状態だけを返す」契約に収まる。
 *
 * シングルスレッド wasm なので `SharedArrayBuffer` も COOP/COEP も要らない。
 */

import init, { HououEngine, version } from '@houou/wasm';
import type { HououInfo, HououResult, WorkerRequest, WorkerResponse } from './protocol.js';

const scope = self as unknown as DedicatedWorkerGlobalScope;

let engine: HououEngine | null = null;

function post(message: WorkerResponse): void {
  scope.postMessage(message);
}

scope.onmessage = (event: MessageEvent<WorkerRequest>): void => {
  const request = event.data;

  switch (request.kind) {
    case 'init':
      void init()
        .then(() => {
          engine = new HououEngine(request.hashMb);
          post({ kind: 'ready', version: version() });
        })
        .catch((error: unknown) => {
          // 初期化に失敗したら `ready` を返さない。UI 側は待ち続けずに
          // 内蔵エンジンへ落ちる（`selectEngine` の既定の振る舞い）。
          post({ kind: 'failed', id: -1, message: String(error) });
        });
      return;

    case 'newGame':
      engine?.newGame();
      return;

    case 'search': {
      if (engine === null) {
        post({ kind: 'failed', id: request.id, message: 'エンジンが初期化されていない' });
        return;
      }
      try {
        const json = engine.search(
          request.sfen,
          request.moves.join(' '),
          request.budgetMs,
          request.maxDepth,
          (infoJson: string) => {
            post({ kind: 'info', id: request.id, info: JSON.parse(infoJson) as HououInfo });
          },
        );
        post({ kind: 'done', id: request.id, result: JSON.parse(json) as HououResult });
      } catch (error: unknown) {
        post({ kind: 'failed', id: request.id, message: String(error) });
      }
      return;
    }
  }
};
