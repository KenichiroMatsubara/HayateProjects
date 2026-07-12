# §8 Web Adapter & Modes

`hayate-adapter-web` の二モード（Canvas Mode / HTML Mode）と、その変更モデル・スタイル写像。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。Canvas/HTML の Element Layer コードは `#[wasm_bindgen]` 専用で native Rust テスト不可。

---

### WEBA-01 — モードはランタイム自動判定
**規範文:** モード選択はランタイム自動判定とする。WebGPU と EditContext API の両方が使えれば Canvas Mode、いずれか欠ければ HTML Mode。判定は host（Hayate web host bootstrap = `@torimi/hayate-host`）が行い、Hayate は Canvas/HTML 両レンダラーを独立に export してアプリは意識しない。
**出典:** ADR-0029, ADR-0037, ADR-0004（host bootstrap の所属確定）, `CONTEXT.md`
**状況:** 🟡 — backend 選択（Vello `hayate-adapter-web` ↔ tiny-skia CPU `hayate-adapter-web-cpu`）の probe は ADR-0004 で Tsubame `renderer-canvas/src/init.ts` から退去し Hayate 側 host bootstrap **`@torimi/hayate-host`（`src/resolve-backend.ts`・`web-host.test.ts` で固定）へ移管**（旧 `init.ts`/`probeWebGPU` は Tsubame に不在）。ただし選択肢は依然 Canvas Mode の2 GPU/CPU backend のみで、**Canvas↔HTML のモード自動判定（EditContext 欠如→HTML フォールバック）は未配線**：`HayateElementHtmlRenderer` は Hayate が export するのみで host bootstrap から到達不能（dead path）。Hayate 内に判定がないのは設計通りだが、host 側の Canvas/HTML 切替が現状は存在しない。
**備考:** ★ 規範文の「いずれか欠ければ HTML Mode」を満たすには `@torimi/hayate-host` の backend resolve に EditContext probe + HTML renderer 経路を追加する必要がある（実装ギャップ、徹底実装フェーズの作業対象）。判定の所在は Tsubame init ではなく Hayate web host bootstrap（ADR-0004 で確定）。

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
| ✅実装済み | 6 | WEBA-02〜07 |
| 🟡部分 | 1 | WEBA-01 |
| ⬜（歴史） | 1 | WEBA-08 |

> Canvas/HTML の Element Layer コードは WASM 専用で native Rust テスト不可（テストは WASM ビルド + JS ランタイム必須）。これは §8 全体に掛かる検証上の制約。
