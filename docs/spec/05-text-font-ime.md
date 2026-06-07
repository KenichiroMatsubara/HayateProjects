# §5 Text / Font / IME

テキストレイアウト（Linebender）、フォント調達、IME 入力。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

## テキスト

### TEXT-01 — Linebender テキストスタック
**規範文:** テキストレイアウトは Linebender スタック（Parley レイアウト / Fontique フォント管理 / Skrifa パース）を採用し、GPU レンダラー Vello と統合する。各クレートは `crates/vendor/` に vendoring する。
**出典:** ADR-0005, ADR-0006
**状況:** ✅ — `crates/vendor/{parley,fontique,skrifa,vello}`、`text.rs` が `parley::{FontContext, Layout, LayoutContext, ...}` を使用。
**備考:** —

### TEXT-02 — バンドルデフォルトフォント
**規範文:** Canvas Mode はシステムフォント不可のため、CJK 対応のデフォルトフォントをバイナリに `include_bytes!` で埋め込み、`ElementTree::new()` で `FontContext` に登録する。未知のファミリ名は CSS フォントスタック `"<requested>, <default>"` でデフォルトにフォールバックする。
**出典:** ADR-0028（canvas-bundled-fonts-default-fallback）, ADR-0005
**状況:** ✅ — `text.rs:17` `DEFAULT_FONT_FAMILY`、`build_text_layout()` が `"{resolved}, {DEFAULT}"` の stack を構成、`resolve_generic_family()` で generic keyword 解決。
**備考:** [衝突 C-8.1] このファイルは番号 `0028` を `0028-html-mode-text-uses-browser-rendering`（§8 WEBA-05）と共有する。意味的衝突ではなく番号衝突。

### TEXT-03 — CJK .notdef 検出による動的調達
**規範文:** glyph run 走査で `.notdef`（`glyph.id==0`）を検出し、コードポイント範囲（CJK / かな / ハングル / アラビア / タイ / デーヴァナーガリー / ヘブライ等）からファミリ名を逆引きして `FetchFont { family }` を発火する。同一フレームの重複は `HashSet` で1回に抑制し、登録後 `fonts_dirty` で全テキストを再シェーピングする。
**出典:** ADR-0042
**状況:** ✅ — `text.rs:38` `codepoint_font_family()`（Unicode ブロックテーブル）、`lower_glyph_runs()` の .notdef 検出、`TextLayout.missing_families`、`event_types.rs` の `FetchFont`。
**備考:** Core はコードポイント→ファミリ名のみ所有（URL は §5 TEXT-04 / adapter）。

### TEXT-04 — フォント URL ディスパッチは Adapter が所有
**規範文:** ファミリ名→CDN URL のマッピングと非同期 fetch は Web Adapter が所有する（Core は所有しない）。`builtin_font_url(family)` テーブルを adapter Rust に持ち、TTF/OTF のみ登録する（fontique/skrifa は WOFF2 非対応）。アプリはフォント URL を書かない。
**出典:** ADR-0043（ADR-0042 の責務分離）
**状況:** ✅ — `element_renderer.rs:26` `builtin_font_url()`、`FontQueue = Rc<RefCell<Vec<(String,Vec<u8>)>>>`、`poll_events()` が `FetchFont` を intercept し `spawn_local` で fetch→queue→`render()` で drain・register。
**備考:** WASM 専用（`spawn_local`）。native Rust ユニットテスト不可。

### TEXT-05 — アプリフォント設定ファイル
**規範文:** アプリのプライマリフォントは `hayate.config` に `[{family, url}]` で宣言し、`configure_fonts()` で描画前にブロッキング preload して FOUT を防ぐ（.notdef 遅延 fetch とは別経路の宣言駆動）。
**出典:** ADR-0044
**状況:** ✅ — `element_renderer.rs:535` `configure_fonts()`（fetch→`tree.register_font`）、HTML Mode 側にも実装。
**備考:** spec プリセット `font_family` enum と app font ID の値域接続は未決（§10 PROTO-19）。

---

## IME

### TEXT-06 — Web Canvas Mode の IME は EditContext 専用
**規範文:** Canvas Mode の IME は EditContext API に統一する。EditContext がないブラウザは HTML Mode にフォールバックし、ブラウザ native の `<input>` に委ねる。
**出典:** ADR-0016
**状況:** ✅ — adapter の `on_composition_start/update/end`（`element_renderer.rs:606`）→ Element Layer へ composition イベント dispatch。
**備考:** モード選択は §8 WEBA-01。

### TEXT-07 — preedit は Element Layer が保持
**規範文:** IME 組成中テキスト（preedit）は `Element::preedit: Option<String>` に保持し、レイアウト時に `text_content + preedit` を合成する（ADR-0058 の text-as-element と整合）。
**出典:** ADR-0017, ADR-0058
**状況:** ✅ — `tree.rs:76` `preedit`、`content_layout`、`layout_pass.rs:270` の合成、`scene_build.rs` の content_layout 描画。Parley editor（vendored）の compose API を利用。
**備考:** Raw Layer ユーザーは IME を自前実装（§4）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 7 | TEXT-01〜07 |
