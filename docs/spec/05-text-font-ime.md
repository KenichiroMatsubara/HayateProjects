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
**出典:** ADR-0073（canvas-bundled-fonts-default-fallback）, ADR-0005
**状況:** ✅ — `text.rs:17` `DEFAULT_FONT_FAMILY`、`build_text_layout()` が `"{resolved}, {DEFAULT}"` の stack を構成、`resolve_generic_family()` で generic keyword 解決。
**備考:** —

### TEXT-03 — CJK .notdef 検出による動的調達
**規範文:** glyph run 走査で `.notdef`（`glyph.id==0`）を検出し、コードポイント範囲（CJK / かな / ハングル / アラビア / タイ / デーヴァナーガリー / ヘブライ等）からファミリ名を逆引きして `FetchFont { family }` を発火する。同一フレームの重複は `HashSet` で1回に抑制し、登録後 `fonts_dirty` で全テキストを再シェーピングする。
**出典:** ADR-0042
**状況:** ✅ — `text.rs:38` `codepoint_font_family()`（Unicode ブロックテーブル）、`lower_glyph_runs()` の .notdef 検出、`TextLayout.missing_families`、`event_types.rs` の `FetchFont`。
**備考:** Core はコードポイント→ファミリ名のみ所有（URL は §5 TEXT-04 / adapter）。

### TEXT-04 — フォント URL ディスパッチは Adapter が所有
**規範文:** ファミリ名→CDN URL のマッピングと非同期 fetch は Web Adapter が所有する（Core は所有しない）。`builtin_font_url(family)` テーブルを adapter Rust に持ち、TTF/OTF のみ登録する（fontique/skrifa は WOFF2 非対応）。アプリはフォント URL を書かない。
**出典:** ADR-0043（ADR-0042 の責務分離）
**状況:** ✅ — `fonts.json` manifest + `build.rs` codegen（`builtin_fonts.rs`）、`FontQueue = Rc<RefCell<Vec<(String,Vec<u8>)>>>`、`poll_events()` が `FetchFont` を intercept し `spawn_local` で fetch→queue→`render()` で drain・register。
**備考:** WASM 専用（`spawn_local`）。`builtin_font_url` の URL カバレッジは native テスト可能（ADR-0061）。

### TEXT-05 — アプリフォント設定ファイル
**規範文:** アプリのプライマリフォントは `hayate.config` に `[{family, url}]` で宣言し、`configure_fonts()` で描画前にブロッキング preload して FOUT を防ぐ（.notdef 遅延 fetch とは別経路の宣言駆動）。
**出典:** ADR-0044
**状況:** ✅ — `element_renderer.rs:535` `configure_fonts()`（fetch→`tree.register_font`）、HTML Mode 側にも実装。
**備考:** spec プリセットと app font はいずれもファミリ名文字列で接続（§10 PROTO-19 / ADR-0061）。

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

### TEXT-08 — text element は inline formatting context（IFC）
**規範文:** `text` element は inline formatting context とする。IFC root（親が `text` でない `text`）は subtree（自身の `el.text` ＋ 子 `text`（inline text element） を document 順）を**1つの Parley ranged layout** として整形する Taffy leaf。inline text element（親が `text` の `text`）は Taffy box を持たず、親 IFC の styled range（font-family/size/weight/style/color/decoration）になる。inline text element への mutation は IFC root の layout を dirty にする。hit-test は IFC root の byte-range→`ElementId` マップで inline text element を解決する。DOM Renderer / HTML Mode はブラウザの native IFC に委ねる。MVP: inline atom（`text` 中の image/icon）は後続、`text-input` は leaf editable のまま、inline text element の box 系スタイルは無視。
**出典:** ADR-0063（ADR-0058 の leaf-string/collapse を supersede、ADR-0005 を拡張）
**状況:** ✅ — hayate-core: `inline_text.rs` の `shape(ifc_root, width)->(Layout, RangeMap)`、`build_ranged_text_layout`、`shape_dirty` 伝播、measure 経路の IFC 合成整形、二段 hit-test（byte→inline text element）。`tsubame-solid`: collapse 撤去済み、text-in-text は `appendChild` + 各 `text` element へ `setText`（`node.ts` は構造のみ）。
**備考:** 現 leaf 整形は inline text element 数=1 の縮退ケースとして IFC 経路に吸収。区間ごとの color は Parley brush（`TextBrush=[u8;4]`）を range push。AccessKit range 化は PLAT-04 下流。

### TEXT-09 — 編集は core の EditState、IME は ImeBridge trait
**規範文:** text-input の編集状態と操作は core の `EditState`（`text_content`/`preedit`/`cursor_byte_index` ＋ insert/append/backspace/set/paste/set_preedit/commit/display_text）に集約する。編集セマンティクス（キー→編集・commit・入力 append）は core が持ち、A1（ADR-0066）で core へ移る入力ハンドラが `EditState` を呼ぶ。platform IME は `ImeBridge` trait の裏に置き、adapter は EditContext（web）/ TSF・TSM・IBus（native）を**ラップするだけ**。core が cursor rect（`cursor_byte_index`＋`content_layout`＋Taffy 由来）を character bounds として `ImeBridge` へ供給し IME 候補窓位置を満たす。`cursor_visible`（点滅・ADR-0032）と `content_layout` は render-side。
**出典:** ADR-0069（ADR-0066/0068 と統合、ADR-0014/0016/0017 を精緻化）
**状況:** ✅ — `edit_state.rs`（`EditState` 集約）、`interaction.rs`（キー/composition/text-input 編集セマンティクス）、`ime_bridge.rs`（`ImeBridge` trait + `CharacterBounds` + `sync_ime_character_bounds`）、`ElementTree::element_character_bounds`、Canvas adapter は `render()` で `WebImeBridge` に bounds 同期＋`ime_character_bounds` export、Tsubame `edit-context-sync.ts` が EditContext へ反映。回帰テスト `edit_input.rs` / `ime_bridge` / `edit-context-sync.test.ts`。
**備考:** IME plumbing は adapter（ImeBridge）、編集 model は core。native は薄い ImeBridge 実装で `EditState`/bounds を再利用（native 本体・ADR-0012）。cursor の点→byte は #3 と共有。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 9 | TEXT-01〜09 |
