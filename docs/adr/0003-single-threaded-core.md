# コアをシングルスレッドで設計する

Hayate のコア（Scene Graph 更新・Layout 計算・Render Command 生成・GPU 送信）を単一スレッドで実行する。

WASM 環境は SharedArrayBuffer なしでは本質的にシングルスレッドであり、マルチスレッド化には COOP/COEP ヘッダーが必要で全環境で保証されない。また wgpu が `!Send` な型を持つため、マルチスレッド化は wgpu の使い方ごと変える必要があり Phase 0 のスコープを超える。

レンダースレッド分離（Flutter/Impeller 方式）は将来の ADR として予約する。API が安定した後、`Send + Sync` 境界を設計する。
