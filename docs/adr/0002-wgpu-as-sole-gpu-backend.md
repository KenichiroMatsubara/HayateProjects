# wgpu を唯一の GPU バックエンドとして採用し、独自抽象層を持たない

Hayate は独自の `GpuBackend` trait を定義せず、wgpu を直接使用する。wgpu はすでに Vulkan / Metal / DX12 / WebGPU（ブラウザ）を抽象化した成熟した GPU 抽象層であり、その上にさらに抽象を重ねることはデバッグの困難化と実装コストの増大を招くだけである。

プラットフォーム対応は wgpu が担う：Web (WASM) → ブラウザ WebGPU、Android → Vulkan、iOS → Metal、Windows → DX12/Vulkan、Linux → Vulkan、macOS → Metal。

## Consequences

WebGPU 仕様で露出されない低レベル GPU 最適化（特定ハードウェア固有の命令等）が将来必要になった場合、Hayate コアを変更するのではなく、**外部拡張として wgpu の native バイナリをさらにコンパイルするレイヤーを Hayate の外側に接続する**方針とする。コアの純粋性を保ちつつ、パフォーマンスの逃げ道を確保する。
