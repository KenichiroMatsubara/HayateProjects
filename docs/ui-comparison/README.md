# UI 比較: DOM モード vs tiny-skia (Canvas) モード

hello-world (`Tsubame/examples/hello-world`) の **Tasks 画面**を、両レンダラーで
描画したときの差分から現行 UI の問題点を洗い出す。

- **DOM Renderer**: Hayate スタイルを実 CSS にマップし、ブラウザがレイアウト・描画する。
- **Canvas Renderer (tiny-skia backend)**: Hayate core がレイアウト・SceneGraph を
  生成し、tiny-skia が CPU ラスタライズする。

## 方法

ブラウザを起動できない実行環境のため、tiny-skia は **ネイティブにヘッドレス描画**できる
ことを利用した。`hayate_core::ElementTree` で Tasks 画面（ライトテーマ + teal アクセント、
`App.tsx` / `theme.ts` に忠実）を組み立て、tiny-skia で PNG に焼く再現ハーネスを追加した。

- ハーネス: `Hayate/crates/scene-renderers/tiny-skia/tests/app_screenshot.rs`
- 実行: `HAYATE_WRITE_SCREENSHOT=1 cargo test -p hayate-scene-renderer-tiny-skia --test app_screenshot -- --nocapture`
  （env 無しでは no-op。CI を増やさない）
- 出力:
  - `tiny-skia-tasks.png` … Tasks 画面全体
  - `tiny-skia-glyphs.png` … グリフ被覆プローブ（`🌙 ☀ ✓ ✕ ↑ ↓ あ A`）

DOM 側はブラウザ標準の CSS セマンティクスを基準とし、tiny-skia 出力との乖離を「問題点」とした。
ボタンのラベルは tsubame-solid 同様に **子 `text` 要素**（ADR-0058）として組み、
ボタンの `defaultColor` / `defaultFontSize` を ambient 継承させて忠実に再現している。

## 一致している点（パリティ OK）

flex レイアウト（row/column・`gap`・`align-items`・`justify-content: space-between`）、
`padding` / `margin`、`border-width` / `border-color` / `border-radius`、`box-shadow`
（パネルの浮き・行の影）、`opacity`（完了行 0.62 のフェード）、配色、日本語本文、
記号グリフ（`✓ ✕ ↑ ↓ ☀`）— いずれも DOM 期待どおりに描画され、レイアウトのズレは無い。

## 問題点（DOM と乖離）

### 1.【High】絵文字 🌙 が Canvas モードで描画されない → テーマ切替ボタンが空白

既定はライトテーマ。AppBar のテーマ切替ボタンはライト時に `🌙`（U+1F319）を表示するが、
tiny-skia ではグリフが無く**ボタンが空白**になる（`tiny-skia-tasks.png` 右上 / `tiny-skia-glyphs.png`
先頭が空）。DOM モードはブラウザの emoji フォントで表示されるため、**主要操作のアイコンが
Canvas モードでだけ消える**重大な乖離。

- 根因: 欠落グリフ→フォント取得の要求が emoji で発火しない（詳細は下記「原因診断」）。
- 補足: ダーク時に出る `☀`（U+2600, dingbat）は NotoSansJP にあり描画される。
  つまり**ライト時のみ**アイコンが消える。
- 対応 issue: #329

### 2.【Medium】日本語の line-break / segmentation model が無く、折返しが DOM と乖離

描画時に大量の `ICU4X data error: No segmentation model for language: ja` が出る。
日本語の行分割モデルが読み込まれておらず、既定の break ルールにフォールバックしている。
note 段落でも `クリック` が `ク` / `リック` のように分割されるなど、**折返し位置が
ブラウザ (DOM) の行分割と一致しない**恐れがある（同じ幅でも改行位置がズレる）。

- 影響: 行数・各行の内容が変わると、コンテナ高さや見た目が Canvas/DOM 間でズレる。
- 対応 issue: #330

### 3.【Note】Button 直接テキストは描画されない（ラベルは子 text が正）

検証中に判明: `ElementKind::Button` に `element_set_text` で**直接**載せたテキストは
描画されない。ラベルは子 `text` 要素として描く必要がある（ADR-0058・tsubame-solid と一致）。
アプリは常に子 text 方式なので実害は無いが、レンダリングモデルの前提として記録する。

### 補足（静的 1 枚では検証不可だが既知の設計上の乖離）

`CONTEXT.md` の通り DOM モードは `opacity` / `transform` で stacking context が
生じ Z-Order が Canvas/Hayate と乖離しうる旨を dev 警告する。完了行の `opacity: 0.62`
はこの条件に該当するため、重なり順が関わる UI を足す際は DOM/Canvas 差に注意。

## 原因診断 (diagnose)

`/diagnose` の規律で上記 2 件を根本原因まで追った。フィードバックループは
`diagnose_glyph_ink`（グリフ単体を描いて ink ピクセル数を数える決定的シグナル）と
描画時の ICU4X ログ。

### 問題 1 の根因: 欠落グリフ→フォント DL の要求が emoji で発火しない

> 当初「fonts.json に emoji が無い」と書いたが、これは**症状**で根本ではない。
> Canvas モードは欠落フォントを**オンデマンドで DL する**仕組みを持つ。真の根因は
> その DL 要求が emoji に対して**一度も発火しない**こと。

`diagnose_glyph_ink` の出力（`HAYATE_WRITE_SCREENSHOT=1` 必須）:

```
[GLYPH-INK]     0 px  U+1F319 🌙 emoji moon      ← 空白（バグ）
[GLYPH-INK]     0 px  U+1F311 🌑 emoji new moon  ← 空白
[GLYPH-INK]   585 px  U+2600 ☀ sun dingbat       ← 描画される（base font 内）
[GLYPH-INK]   138 px  U+263D ☽ /  148 px U+263E ☾ /  169 px ✓ /  234 px ✕ / ク A
```

**DL 経路（設計）**: `core/.../text.rs::lower_glyph_runs` が `.notdef`(glyph id 0) を検知
→ その codepoint を `codepoint_font_family(cp)` で補完フォント名に引く → `missing_families`
→ `layout_pass` が `Event::FetchFont{family}` を発火 → web アダプタ
`canvas.rs::poll_events` が `builtin_font_url(family)` で URL を引き fetch → `font_queue`
→ 次フレームの `render()` 冒頭で `register_font` → 再レイアウトで正しいグリフ。
CJK (Noto Sans JP) はこの経路で実際に動いている。

**根因**: `core/.../text.rs::codepoint_font_family` の範囲表に **emoji の arm が無い**
（CJK / ハングル / アラビア / タイ / デーヴァナーガリー / ヘブライのみ。emoji は `_ => None`）。
よって 🌙 (U+1F319) は補完フォント名が `None` → `FetchFont` が**発火せず** → DL もされず空白。
☀ (U+2600) が出るのは base font (NotoSansJP) に元から有り `.notdef` にならないため。
DOM はブラウザが OS emoji へ fallback するため表示され、ここが乖離。

**tiny-skia 制約**: painter は `outline_glyphs()`（glyf/CFF アウトライン）のみ描画し、
COLR/CBDT の**カラー emoji は描けない**。よって補完先は**モノクロの "Noto Emoji"**で
なければならず、"Noto Color Emoji" では tiny-skia で依然空白になる。

→ 修正方針と受け入れ条件は **issue #329** に集約。本ドキュメントは分析のみで、コード変更は含まない。

### 問題 2 の根因: parley が CJK 辞書を読み込まない line segmenter を使用

ICU4X ログ `No segmentation model for language: ja` の発生源は
`icu_segmenter-2.2.0/src/complex/mod.rs::select()`：`ChineseOrJapanese` に対し
`cjdict`(CJK 辞書) payload が `None` だと当該エラーを出し、CJK 連を 1 セグメント扱いに
フォールバックする（＝辞書による語境界を使わず UAX#14 既定で kana 間を切る）。

なぜ `None` か。vendored parley の
`crates/vendor/parley/src/analysis/mod.rs::line_segmenter()` が **全 word-break モードで
`LineSegmenter::new_for_non_complex_scripts(opt)` を使う**ため、複雑スクリプト用 (CJK/東南
アジア) の辞書データを一切ロードしない。結果、日本語は辞書非対応で UAX#14 既定折返しになり、
`クリック` が `ク`/`リック` のように分割され、ブラウザ(DOM)の ICU 辞書折返しと改行位置が乖離する。

→ 対応方針の検討と受け入れ条件は **issue #330** に集約。

### 回帰シグナル

`diagnose_glyph_ink`（ink 数）と `render_glyph_coverage` / `render_tasks_screen`（目視）を
そのまま回帰確認に使える。問題 1 を修正したら `🌙` ではなく採用 dingbat の ink が
0 でないこと、Tasks 画面でトグルにアイコンが出ることを確認する。

## スクリーンショット

![Tasks 画面 (tiny-skia)](./tiny-skia-tasks.png)

![グリフ被覆 (tiny-skia)](./tiny-skia-glyphs.png)
