# プラットフォーム非依存の Render Host / Font ロードを共有層へ hoist する（ADR-0054 H1 を revisit）

**Status: accepted（ADR-0054 H1 の deferral を revisit。候補 D2 / ADR-0066 を継承）**

**Date: 2026-06-07**

## Context

`hayate-adapter-web` の `element_renderer.rs`（1593行）は、**プラットフォーム非依存の concern と web 特有の concern を1 struct に混在**させている（候補 D2）。ADR-0066（A1）が interaction 状態機械を core へ移し最大の塊は片付いたが、Render Host（renderer 選択・present・resize・資源寿命）と Font 非同期ロード（queue・drain→register）が web adapter に残る。これらは芯がプラットフォーム非依存で、殻（surface 生成・font fetch）だけが web 特有。

ADR-0054 H1 / decisions-pending #2 は「web surface の共有層移管は **native adapter 追加時まで deferred**」とした。LANGUAGE.md も「1 adapter＝仮説の seam、2 adapter＝本物の seam」と投機的 seam を戒める。

**しかし native は仮説ではなく確定した設計目標である**：CONTEXT.md は WIT が web/native デュアルターゲットにコンパイルされ「品質は等階級」と定める。Hayate の存在意義自体が web ＋ native。したがって `Surface` / `FontFetcher` の variation は **roadmap 上で確定**しており、「起きないかもしれない変化」ではない。2-adapter 則の目的（無駄な seam の回避）はここでは当てはまらない。**確定した第2ターゲットのために seam を今引くのは投機ではなく前払い**である。

## Decision

**プラットフォーム非依存の Render Host と Font ロードを共有層（`hayate-render-host` crate、または core 上位）へ hoist し、web 特有部分を platform trait の裏に置く。** ADR-0054 H1 の deferral を revisit する。

### 共有層（platform 非依存）

- **Render Host**：Renderer Selection Policy（ADR-0050）・render orchestration（`render_scene_graph` 駆動、ADR-0054）・資源寿命・fallback。
- **`Surface` trait**：wgpu surface の取得・`present`・`resize`・`configure`。surface の**生成**は platform 実装が供給。
- **Font ロード**：queue 管理・drain→`tree.register_font`・`.notdef` fetch 要求の受理。
- **`FontFetcher` trait**：`fetch(url) -> bytes`。実体は platform 実装。
- interaction 状態機械は既に core（ADR-0066）。

### web 特有（`hayate-adapter-web` に残す）

- `#[wasm_bindgen]` binding surface（`JsValue` / f64↔ElementId 翻訳）。
- `impl Surface`（`HtmlCanvasElement` → wgpu surface）。
- `impl FontFetcher`（web `fetch` ＋ `spawn_local`）。
- EditContext IME（ADR-0016）・HTML Mode（web 専用経路）。

### 将来の native adapter

- `impl Surface`（native window / raw-window-handle）＋ `impl FontFetcher`（http/fs）＋ wit-bindgen binding ＋ native IME（TSF/TSM/IBus）。共有層をそのまま再利用。

## Consequences

- web adapter が「真に web 特有なもの（wasm binding・canvas surface・fetch・IME・HTML mode）」へ縮む。
- native adapter は薄い新 adapter（2 trait の実装＋binding）になり、Render Host/Font/状態機械/描画を**再実装しない**。
- `Surface` / `FontFetcher` という platform seam を**今**導入する（web 実装のみ）。
- **受容するリスク**：web 単独実装は trait 形を native で検証できないため、native 到来時に trait 調整の手戻りがありうる。最小 trait（Surface: acquire/present/resize/configure、FontFetcher: fetch）に保ち露出を抑える。
- ADR-0054 H1（surface は当面 web adapter 残留）を **revisit**：芯を今 hoist する。surface **生成**は platform 実装として web に残るので、ADR-0054 H1 の「surface 生成は web」という事実自体は trait の裏で維持。

## Considered Options

- **(a) adapter 内抽出のみ（cross-crate seam は native 到来まで作らない）**：ADR-0054 H1・2-adapter 則に忠実。だが native が確定目標である以上、芯の共有化を遅らせる積極的理由が薄い。却下（本決定で (b) を採用）。
- **(b) 共有層へ hoist（本決定）**：native を前払い。web adapter が縮み native が薄くなる。trait 手戻りリスクを受容。

## 関係

- ADR-0054 H1 / decisions-pending #2：revisit（deferral を解除し芯を hoist）。
- ADR-0050：Renderer Selection Policy / Render Host 語彙を共有層で実体化。
- ADR-0066（A1）：interaction 状態機械の core 移管に続く platform 非依存化の第2弾。
- ADR-0014：Platform Adapter（IME/clipboard/input）の責務境界と整合（surface/fetch も platform trait 化）。
- ADR-0002（wgpu 唯一の Backend）：wgpu がクロスプラットフォームなので共有 Render Host は wgpu をそのまま使える。
