# §1 Hayate Core 原則

Hayate Core（Element Layer・Scene Graph・Layout・Render Command 生成）の基盤決定。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### CORE-01 — Rust をコア実装言語とする
**規範文:** Hayate Core は Rust で実装し、cargo workspace で統一する。
**出典:** ADR-0001
**状況:** ✅ — `Hayate/Cargo.toml` workspace（core / scene-renderers / adapters / proto/generator は全て Rust crate）。
**備考:** C ABI（旧 newdom.h）は廃止。

### CORE-02 — wgpu を唯一の GPU API とする
**規範文:** Hayate Core は独自 GPU 抽象 trait を定義せず、GPU API は wgpu に一本化する。Vulkan / Metal / DX12 / WebGPU への変換は wgpu が担う。
**出典:** ADR-0002
**状況:** ✅ — Core は wgpu を直接 import せず GPU 非依存。GPU 描画は scene-renderer/vello（wgpu 依存）経由のみ。
**備考:** [整理] tiny-skia は GPU バックエンドではなく CPU フォールバック（ADR-0048, §4 REND-02）。「wgpu 唯一」は GPU 層の規範であり tiny-skia と矛盾しない。

### CORE-03 — シングルスレッド設計
**規範文:** Core（Element Layer・Scene Graph 更新・Layout・Render Command 生成）は単一スレッドで実行する。共有可変状態は WASM 前提で `Rc<RefCell>`、`Arc` は immutable データ共有に限る。
**出典:** ADR-0003
**状況:** ✅ — `crates/core/src` に `thread::spawn` / `tokio` / `async` なし。`Arc` 使用は `Arc<str>` / `Arc<TextRunData>` / `Arc<RenderImage>` の immutable のみ。adapter は `Rc<RefCell>`。
**備考:** マルチスレッド化は将来 ADR の予約事項。

### CORE-04 — 主要依存を vendoring する
**規範文:** Vello・Taffy・Parley・Fontique・Skrifa を `crates/vendor/` に取り込み `[patch.crates-io]` で上書きし、upstream から自律した upgrade cycle を確保する。wgpu は巨大・platform 追従コストのため対象外。
**出典:** ADR-0007
**状況:** ✅ — `Hayate/Cargo.toml [patch.crates-io]` に vello/taffy/parley/fontique/skrifa の path 指定、`crates/vendor/` に各ディレクトリ存在。wgpu は version 指定のまま。
**備考:** —

### CORE-05 — プラットフォーム等階級（Web は最初の実装）
**規範文:** Hayate は全プラットフォームを等階級とし、Web は「最初の実装」と位置づける。Core は platform-agnostic で Platform Adapter を知らない。
**出典:** ADR-0012
**状況:** ✅ — `crates/core` に Web 固有型・Platform Adapter 型なし。`adapters/web` が Core に片方向依存。
**備考:** 等階級の実証（IME/clipboard/a11y の Web vs native 同質性）は native adapter 実装時（§9）に持ち越し。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 5 | CORE-01〜05 |
