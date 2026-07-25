# §8 Web Adapter & Modes

`hayate-adapter-web` の二モード（Canvas Mode / HTML Mode）と、その変更モデル・スタイル写像。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。Canvas/HTML の Element Layer コードは `#[wasm_bindgen]` 専用で native Rust テスト不可。

---

### WEBA-01 — Canvas Mode は標準 Worker 経路
**規範文:** Web entry は明示 `?renderer=dom` または EditContext 欠如時だけ HTML Mode を選び、それ以外は Canvas Mode とする。Canvas Mode は opt-in flag なしで OffscreenCanvas＋単一 Worker を使い、Worker が Core・Render Host・Scene Renderer・共通 latest-wins frame pipeline・surface を同じ lifetime で所有する。main thread は DOM input / IME / clock transport のみを担う。
**出典:** ADR-0029, ADR-0037, ADR-0004, ADR-0128, ADR-0157
**状況:** ✅ — `@torimi/hayate-host` の `createHayateWebHost` は必ず一つの module Worker を起動して canvas を transfer する。Worker/OffscreenCanvas 不在と renderer 初期化失敗は typed boot failure であり、main-thread Canvas fallback はない。初回 renderer 選択は Worker 内の Rust Render Host が **Vello → tiny-skia** の順で行い、初回 init failure のときだけ次候補へ進む。選択後の render / surface / context failure は terminal で、renderer/Worker を再起動しない。production と test は同じ host 経路を使い、renderer/Worker query flag と旧 oracle は持たない。
**備考:** HTML Mode は独立経路のまま変更しない。Tsubame app パッケージは DOM への明示 escape と EditContext の二値判定だけを持ち、Canvas renderer の名前や選択結果を持たない。

### WEBA-02 — Canvas Mode は eager 変更
**規範文:** Canvas Mode の変更（`element_create` / `element_set_style` / `element_append_child` 等）は `ElementTree` に即時反映する（遅延キューなし）。Tsubame が1フレーム分を JS 側でバッチ化し `apply_mutations` 1回で渡す。
**出典:** ADR-0037（Canvas の遅延キューを撤去）
**状況:** ✅ — `HayateElementRenderer`（`element_renderer.rs:137`）の setter が `self.tree.*()` を直接呼ぶ。`apply_mutations`（`:504`）が batch を eager 処理。
**備考:** [履歴] ADR-0030 の遅延キューは ADR-0037 で Canvas から撤去。

### WEBA-03 — HTML Mode は遅延コマンドキュー
**規範文:** HTML Mode の変更は `Vec<Command>` に積み、`render()` を唯一のフラッシュ境界として `flush_pending()` で DOM に一括適用する（レイアウトスラッシング回避）。
**出典:** ADR-0030（HTML に scope 縮小）, ADR-0037
**状況:** ✅ — `HayateElementHtmlRenderer`（`element_renderer.rs:592`）の `pending: Vec<Command>`（`Command` enum `:36`）、setter が `Command::*` を enqueue、`render()`（`:769`）→`flush_pending()`（`:1067`）。
**備考:** Canvas（eager）/ HTML（deferred）の二モデルが単一ファイルに同居（アーキテクチャレビュー候補2、§改善）。

### WEBA-04 — HTML Mode は Hayate CSS → ブラウザ CSS 1:1 写像
**規範文:** HTML Mode は Hayate CSS をブラウザ CSS に 1:1 マッピングし、レイアウト計算をブラウザ CSS エンジンに委ねる（Taffy 不経由）。絶対配置 div 方式は採らない。
**出典:** ADR-0029（browser-css-layout）
**状況:** ✅ — `style_packet.rs:17` `apply_props_to_dom()`→生成 `dom_style_mapper.rs`（background-color/display/flex-direction/gap 等の 1:1 写像）。HTML 経路に Taffy なし。`inject_baseline_stylesheet()` で box-sizing 等を正規化。
**備考:** —

### WEBA-05 — HTML Mode のテキストはブラウザ native 描画
**規範文:** HTML Mode のテキストは `set_inner_text()` でブラウザ native 描画に委ね、font-family/size/color を CSS で設定する。Parley/Vello/fontique/skrifa は HTML Mode では呼ばない。
**出典:** ADR-0028（html-mode-text-uses-browser-rendering）
**状況:** ✅ — `flush_set_text_content():1410`（`set_inner_text`）、`flush_set_font_family():1388`。HTML 経路に Parley/Vello なし。
**備考:** Canvas（Parley+Vello）と HTML（ブラウザ）でテキスト品質が異なるのは ADR-0028 が受容した設計。

### WEBA-06 — HTML Mode の z-index は CSS プロパティ直書き
**規範文:** HTML Mode は CSS `z-index` を要素に直接設定し、stacking はブラウザ CSS エンジンに委ねる（`ordered_children` による再ソートは行わない）。
**出典:** ADR-0029（browser-css-layout）, ADR-0060（z-order seam）
**状況:** ✅ — `dom_style_mapper.rs` の `ZIndex(z)`→`z-index` 設定。`walk_resolved` は document order のまま（ADR-0060）。
**備考:** [履歴] 旧絶対座標レイヤー方式の z-index 記述は ADR-0074（superseded）。Canvas Mode の子再ソート（§4 REND-03）とは別経路。

### WEBA-07 — HTML Mode のレイアウト差異は許容
**規範文:** HTML Mode は CSS セマンティクス（transform/opacity の stacking、z-index scope）で Canvas Mode と差異が出る。これは「開発時 UI 確認」「非 Chromium フォールバック」用途であり、ピクセル完全一致は目標にしない。
**出典:** ADR-0029（Known Limitations）
**状況:** ✅ — 受容した設計境界。
**備考:** ピクセル完全一致は Canvas Mode（同一フォントバンドル時）の保証。

### WEBA-08 — ADR-0010 / 0011 は歴史的
**規範文:** ブラウザ computed layout 抽出（getBoundingClientRect）と CSS エンジンバンドルの旧アプローチは現行実装では採らない。
**出典:** ADR-0010（→0011 で superseded）, ADR-0011（scope 撤回で historical）
**状況:** ⬜（歴史） — 該当実装なし。HTML Mode は WEBA-04 のブラウザ CSS 委譲に収束。
**備考:** —

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 7 | WEBA-01〜07 |
| ⬜（歴史） | 1 | WEBA-08 |

> Canvas/HTML の Element Layer コードは WASM 専用で native Rust テスト不可（テストは WASM ビルド + JS ランタイム必須）。これは §8 全体に掛かる検証上の制約。
