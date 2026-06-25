# Context Map

This monorepo has multiple domain contexts. Read the `CONTEXT.md` relevant to the area you are working in.

| Context | Path | Scope |
| ------- | ---- | ----- |
| **Hayate / Hayabusa (ecosystem)** | [`CONTEXT.md`](./CONTEXT.md) | Cross-cutting product language, Element Layer, Hayabusa SFC framework |
| **Hayate (Rust/WASM core)** | [`Hayate/CONTEXT.md`](./Hayate/CONTEXT.md) | Element Document Runtime, SceneGraph, adapters, Hayate CSS |
| **Tsubame (JS/TS renderer)** | [`Tsubame/CONTEXT.md`](./Tsubame/CONTEXT.md) | Renderer Protocol, DOM/Canvas renderers, tsubame-solid |

## Context relationships (dependency boundary)

- **Hayate → Tsubame: 依存なし（永久）.** Hayate は Tsubame を知らない（ADR-0001 維持）。
- **Tsubame → Hayate: Contract 経由のみ.** Tsubame は `@hayate/protocol-spec`（Protocol Contract）と、自前定義の `RawHayate` ポート型だけを通じて Hayate に触れる。Hayate の**ランタイム/WASM adapter パッケージ（`hayate-adapter-web` 等）には依存しない**。具体 adapter は **App（合成ルート）が注入**する。
- **App → Tsubame + Hayate ランタイム.** host bootstrap（surface 取得・WASM ロード・backend 選択・clock 源・native glue）は **Hayate ランタイム側または App** が持ち、**Tsubame の renderer パッケージには置かない**（docs/adr/0004）。

## ADR layout

| Scope | Location |
| ----- | -------- |
| Monorepo / system-wide | [`docs/adr/`](./docs/adr/) |
| Hayate-specific | [`Hayate/docs/adr/`](./Hayate/docs/adr/) |
| Hayabusa-specific | [`Hayabusa/docs/adr/`](./Hayabusa/docs/adr/) |
| Tsubame-specific | [`Tsubame/docs/adr/`](./Tsubame/docs/adr/) |
