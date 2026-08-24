import type { ShogiEngine, ShogiEnginePlugin } from './engine.js';
import { builtinEnginePlugin } from './builtin/builtin-engine.js';
import { hououEnginePlugin } from './houou/houou-engine.js';

/**
 * 使うエンジンを**実行時に**選ぶ。ビルド時のターゲット分岐は置かない —
 * バンドルは全ターゲットで 1 本のまま保つ（ADR-0008 §4 の流儀）。
 *
 * 既製の強エンジン（YaneuraOu 等）は GPL-3.0 なので**リポジトリにも配布物にも含めない**。
 * 使う場合は実行時に別オリジンから読む形にし、この選択がそのままライセンス境界になる。
 * したがってここに既製強エンジンの静的 import は**あってはならない**。
 *
 * Houou（`Houou/`）は自作 = Apache-2.0 なので、この制約に触れない。同梱してよい。
 */

/**
 * 候補は前から順に見て、最初に使えるものを採る。既定は最後の砦。
 *
 * - `houou` — 自作 Rust エンジンの wasm。`WebAssembly` と `Worker` が要る。
 * - `builtin` — 純 TS。ブラウザでも埋め込み Hermes でも動く最後の砦。
 */
const PLUGINS: readonly ShogiEnginePlugin[] = [hououEnginePlugin, builtinEnginePlugin];

/**
 * この実行環境で使えるエンジンを 1 つ返す。どのプラグインの生成が失敗しても、
 * 最終的に内蔵エンジンへ落ちて**例外を投げない**（対局が始まらない事態を作らない）。
 */
export async function selectEngine(
  plugins: readonly ShogiEnginePlugin[] = PLUGINS,
): Promise<ShogiEngine> {
  for (const plugin of plugins) {
    if (!plugin.isAvailable()) continue;
    try {
      return await plugin.create();
    } catch {
      // この候補は使えなかった。次を試す。
    }
  }
  return builtinEnginePlugin.create();
}
