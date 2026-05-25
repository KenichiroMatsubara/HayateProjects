# Rust をコア実装言語として採用する

仕様書は C または Zig を候補としていたが、GPU バックエンドに wgpu を採用することを決定した結果、Rust を選択した。wgpu は Rust ネイティブのライブラリであり、wgpu を使いながらコアを Zig で書くとビルドシステムが zig build + cargo の二重管理になる。Rust で統一することで cargo 一本に収まり、クロスコンパイル・WASM（wasm-pack）・C ABI 生成（cbindgen）が一貫したツールチェーンで完結する。

## Considered Options

- **C**: FFI 互換性は最高だが、wgpu を C から呼ぶには wgpu-native の C ABI 経由となりビルドが複雑化する。メモリ安全性も手動。
- **Zig**: クロスコンパイルが強力だが、wgpu（Rust）との共存でビルドシステムが二重になる。
- **Rust**: wgpu とネイティブ統合、cargo 一本、borrow checker による安全性、OSS コントリビューター母数が最大。

公開インターフェースは WIT（WebAssembly Interface Types）として定義し、wit-bindgen で各言語の SDK を自動生成する（ADR-0015）。C ABI（newdom.h）は廃止。
