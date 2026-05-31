# Tsubame を Hayate と完全に独立した別リポジトリとする

> **Superseded（リポジトリ構造のみ）**: HayateProjects モノレポへ統合済み（root/docs/adr/0001-monorepo-migration.md）。
> アーキテクチャ上の分離（Hayate は Tsubame を知らない・結合点は apply_mutations のみ）は引き続き有効。

_origin: Hayate ADR-0035_

Hayabusa は当初 JS（TypeScript）を Script Adapter の一つとして扱い、Hermes 等の JS エンジンをモバイルでバンドルする方向だった。しかしこの設計では JS→WASM 境界（Hayabusa Script Adapter → Hayabusa Runtime WASM → Hayate WASM）が毎フレーム N 回発生し、deferred queue で WASM→JS 側を最適化しても JS→WASM 側は未解決のままだった。

JS サポートを Tsubame という完全独立の JS フレームワークとして切り出し、Hayabusa は純粋 WASM 専用（Rust / Python 等のコンパイル言語向け）とする。Hayabusa と Hayate は単一 WASM バイナリにリンクされ（Rust クレート依存）、層間の境界コストはゼロになる。Tsubame は JS→WASM 境界を持つが、Canvas Mode では `apply_mutations` で 1回/frame に集約する（ADR-0003 参照）。

## 採用した設計

- Tsubame は Hayate・Hayabusa とは独立した pure JS モノレポ
- 結合点は `apply_mutations(ops: Float64Array, styles: Float32Array)` の仕様のみ
- Hayate・Hayabusa コアは Tsubame の存在を知らない

## Considered Options

- Hayabusa が JS も扱い続ける: JS→WASM 境界コストが未解決のまま残り、Hermes バンドルも必要
- WIT component model を使い続ける: Hayabusa と Hayate が別 WASM コンポーネントのまま canonical ABI オーバーヘッドが残る
