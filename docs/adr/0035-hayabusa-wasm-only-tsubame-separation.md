# Hayabusa を純粋 WASM 専用とし JS を Tsubame として独立させる

Hayabusa は当初 JS（TypeScript）を Script Adapter の一つとして扱い、Hermes 等の JS エンジンをモバイルでバンドルする方向だった。しかしこの設計では JS→WASM 境界（Hayabusa Script Adapter → Hayabusa Runtime WASM → Hayate WASM）が毎フレーム N 回発生し、deferred queue で WASM→JS 側を最適化しても JS→WASM 側は未解決のままだった。

JS サポートを Tsubame という完全独立の JS フレームワークとして切り出し、Hayabusa は純粋 WASM 専用（Rust / Python 等のコンパイル言語向け）とする。Hayabusa と Hayate は単一 WASM バイナリにリンクされ（Rust クレート依存）、層間の境界コストはゼロになる。Tsubame は JS→WASM 境界を持つが、Canvas Mode では apply_mutations で 1 回/frame に集約する（ADR-0036 参照）。

## Considered Options

- Hayabusa が JS も扱い続ける: JS→WASM 境界コストが未解決のまま残り、Hermes バンドルも必要になる
- WIT component model を使い続ける: Hayabusa と Hayate が別 WASM コンポーネントのまま canonical ABI オーバーヘッドが残る
