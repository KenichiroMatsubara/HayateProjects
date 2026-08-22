/**
 * 強エンジン（WASM）が動く環境かを実行時に見る。
 *
 * 実測に基づく前提:
 * - 埋め込み Hermes（ネイティブホスト）には `WebAssembly` も `Worker` も**無い**。
 * - 強い将棋エンジンの WASM ビルド（`@mizarjp/yaneuraou.*` / `fairy-stockfish-nnue.wasm`）は
 *   emscripten の `-pthread` で **wasm メモリを `shared` と宣言**しており、
 *   `Threads 1` にしても回避できない（ビルド時の性質）。
 *   したがって **`COOP: same-origin` + `COEP: require-corp` によるクロスオリジン分離が
 *   無いとインスタンス化に失敗する**（優雅な劣化はしない）。
 */

export function hasWebAssembly(): boolean {
  const wasm = (globalThis as { WebAssembly?: { instantiate?: unknown } }).WebAssembly;
  return typeof wasm === 'object' && wasm !== null && typeof wasm.instantiate === 'function';
}

export function hasWorker(): boolean {
  return typeof (globalThis as { Worker?: unknown }).Worker === 'function';
}

/** SharedArrayBuffer を使う wasm を載せられるか（COOP/COEP 済みか）。 */
export function isCrossOriginIsolated(): boolean {
  return (globalThis as { crossOriginIsolated?: boolean }).crossOriginIsolated === true;
}

/** 強エンジンを載せられる環境か。3 つすべてが要る。 */
export function canRunStrongEngine(): boolean {
  return hasWebAssembly() && hasWorker() && isCrossOriginIsolated();
}
