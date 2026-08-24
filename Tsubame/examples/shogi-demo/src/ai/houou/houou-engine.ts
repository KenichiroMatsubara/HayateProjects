import type {
  EngineProgress,
  EngineSearch,
  EngineSearchRequest,
  ShogiEngine,
  ShogiEnginePlugin,
} from '../engine.js';
import { hasWebAssembly, hasWorker } from '../strong/capability.js';
import type { HououInfo, WorkerRequest, WorkerResponse } from './protocol.js';

/**
 * Houou — 自作の Rust エンジン（`Houou/`）を wasm で回すプラグイン。
 *
 * ## 内蔵 TS エンジンとの違い
 *
 * 内蔵は tsshogi の合法手生成に律速されて深さ 3 が限界だが、こちらは自前の
 * ビットボード生成と置換表・PVS・静止探索を持ち、同じ 1.2 秒で深さ 10 前後まで読む。
 *
 * ## 要る条件は `WebAssembly` と `Worker` の 2 つだけ
 *
 * **`crossOriginIsolated` は要らない。** `capability.ts` が挙げている COOP/COEP は
 * 既製の強エンジン（emscripten の `-pthread` ビルドで wasm メモリが `shared`）の話で、
 * 自前のシングルスレッド wasm には当たらない。
 *
 * 埋め込み Hermes（Torimi ネイティブ）には `WebAssembly` も `Worker` も無いので
 * `isAvailable` が偽になり、内蔵エンジンへ落ちる。ネイティブで強いエンジンを動かすのは
 * 別経路（`houou-core` をホストへ静的リンクする Phase 5）の仕事。
 */

export const HOUOU_ENGINE_ID = 'houou';

/** 置換表の大きさ。ブラウザのメモリを踏まえて控えめに取る。 */
const HASH_MB = 32;

/**
 * wasm に渡す予算は、呼ぶ側の予算より少し短くする。
 *
 * `driver.ts` は予算を過ぎると `cancel()` して**その場で `poll` に `done` を求める**が、
 * Worker の中で走っている探索を外から止める術は無い（シングルスレッド wasm には
 * 割り込みが無い）。先に自分から終わってもらうのが唯一の手。
 */
const SAFETY_MARGIN_MS = 150;

/** Worker の起動と初期化を待つ上限。 */
const READY_TIMEOUT_MS = 5_000;

class HououSearch implements EngineSearch {
  private progress: EngineProgress = { kind: 'thinking', depth: 0, nodes: 0 };
  /** 最後に受け取った進捗。打ち切られたときの落としどころになる。 */
  private latest: HououInfo | null = null;
  private settled = false;

  constructor(
    private readonly engine: HououEngineImpl,
    readonly id: number,
  ) {}

  /** Worker から進捗が届いた。 */
  onInfo(info: HououInfo): void {
    if (this.settled) return;
    this.latest = info;
    this.progress = { kind: 'thinking', depth: info.depth, nodes: info.nodes };
  }

  /** Worker から結末が届いた。 */
  onDone(progress: Extract<EngineProgress, { kind: 'done' }>): void {
    this.settled = true;
    this.progress = progress;
  }

  poll(): EngineProgress {
    return this.progress;
  }

  cancel(): void {
    if (this.settled) return;
    // Worker を止められないので、**今までに届いた最善手で確定させる**。
    // 予算に余裕を持たせてあるので、ここに来る前に `done` が届いているのが普通。
    this.settled = true;
    this.progress = {
      kind: 'done',
      bestMove: this.latest?.pv[0] ?? null,
      scoreCp: this.latest?.scoreCp ?? 0,
      depth: this.latest?.depth ?? 0,
      nodes: this.latest?.nodes ?? 0,
    };
    this.engine.forget(this.id);
  }
}

class HououEngineImpl implements ShogiEngine {
  readonly id = HOUOU_ENGINE_ID;
  readonly displayName = 'Houou';

  private nextId = 1;
  private readonly running = new Map<number, HououSearch>();

  constructor(
    private readonly worker: Worker,
    readonly version: string,
  ) {
    this.worker.addEventListener('message', this.onMessage);
  }

  private readonly onMessage = (event: MessageEvent<WorkerResponse>): void => {
    const message = event.data;
    if (message.kind === 'ready') return;

    const search = this.running.get(message.id);
    // 打ち切ったあとに届いた応答。捨てる。
    if (search === undefined) return;

    switch (message.kind) {
      case 'info':
        search.onInfo(message.info);
        return;
      case 'done':
        search.onDone({
          kind: 'done',
          bestMove: message.result.bestMove,
          scoreCp: message.result.scoreCp,
          depth: message.result.depth,
          nodes: message.result.nodes,
        });
        this.running.delete(message.id);
        return;
      case 'failed':
        // 読めない局面などで探索が立たなかった。手を返さずに終える
        // （呼ぶ側から見れば「合法手が無い」と同じ扱いになる）。
        search.onDone({ kind: 'done', bestMove: null, scoreCp: 0, depth: 0, nodes: 0 });
        this.running.delete(message.id);
        return;
    }
  };

  forget(id: number): void {
    this.running.delete(id);
  }

  newGame(): void {
    this.post({ kind: 'newGame' });
  }

  search(request: EngineSearchRequest): EngineSearch {
    const id = this.nextId++;
    const search = new HououSearch(this, id);
    this.running.set(id, search);

    this.post({
      kind: 'search',
      id,
      sfen: request.sfen,
      moves: [...request.moves],
      budgetMs: Math.max(50, request.budgetMs - SAFETY_MARGIN_MS),
      maxDepth: request.maxDepth ?? 0,
    });

    return search;
  }

  dispose(): void {
    this.worker.removeEventListener('message', this.onMessage);
    this.worker.terminate();
    this.running.clear();
  }

  private post(request: WorkerRequest): void {
    this.worker.postMessage(request);
  }
}

/**
 * Worker を起こして wasm の初期化を待つ。`ready` が来なければ拒否して、
 * 呼ぶ側（`selectEngine`）が内蔵エンジンへ落ちられるようにする。
 */
async function createEngine(): Promise<ShogiEngine> {
  const worker = new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' });

  const version = await new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => {
      cleanup();
      reject(new Error('Houou の初期化が返ってこない'));
    }, READY_TIMEOUT_MS);

    const onMessage = (event: MessageEvent<WorkerResponse>): void => {
      if (event.data.kind === 'ready') {
        cleanup();
        resolve(event.data.version);
      } else if (event.data.kind === 'failed') {
        cleanup();
        reject(new Error(event.data.message));
      }
    };

    function cleanup(): void {
      clearTimeout(timer);
      worker.removeEventListener('message', onMessage);
    }

    worker.addEventListener('message', onMessage);
    worker.postMessage({ kind: 'init', hashMb: HASH_MB } satisfies WorkerRequest);
  }).catch((error: unknown) => {
    worker.terminate();
    throw error;
  });

  return new HououEngineImpl(worker, version);
}

export const hououEnginePlugin: ShogiEnginePlugin = {
  id: HOUOU_ENGINE_ID,
  isAvailable: () => hasWebAssembly() && hasWorker(),
  create: createEngine,
};
