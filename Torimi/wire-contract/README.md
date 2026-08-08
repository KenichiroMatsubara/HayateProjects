# @torimi/wire-contract

Torimi の route、message、global / property 名、open/closed vocabulary、Device Log、Demo Manifest の言語中立な wire contract です（ADR-0006）。

- `spec/vocabulary.json` と `spec/wire.schema.json` が唯一の正本です。
- `pnpm generate` が TypeScript、Android Native Host 用 Rust、Hermes JSI 用 C++ projection を生成します。
- `src/generated.ts`、`Hayate/crates/platform/mobile/android/src/generated/torimi_wire.rs`、`Hayate/crates/platform/mobile/android/cpp/generated/torimi_wire.hpp` は手編集しません。
- protocol version 比較、reload backoff、timeout、buffer、boot lifecycle は behavior なので、この package には置きません。

リポジトリ root の `pnpm check:wire-contract` は再生成の byte parity、TypeScript validator と Rust DTO の fixture parity、C++ header compile を検査します。
