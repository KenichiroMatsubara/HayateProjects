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

- 根因: バンドルフォント (NotoSansJP) に emoji グリフが無く、emoji フォントへの
  フォールバック経路が無い。
- 補足: ダーク時に出る `☀`（U+2600, dingbat）は NotoSansJP にあり描画される。
  つまり**ライト時のみ**アイコンが消える。
- 対策案: emoji フォントの登録／フォールバック、または切替アイコンを dingbat や
  `text`/SVG ベースのアイコンに変更する。

### 2.【Medium】日本語の line-break / segmentation model が無く、折返しが DOM と乖離

描画時に大量の `ICU4X data error: No segmentation model for language: ja` が出る。
日本語の行分割モデルが読み込まれておらず、既定の break ルールにフォールバックしている。
note 段落でも `クリック` が `ク` / `リック` のように分割されるなど、**折返し位置が
ブラウザ (DOM) の行分割と一致しない**恐れがある（同じ幅でも改行位置がズレる）。

- 影響: 行数・各行の内容が変わると、コンテナ高さや見た目が Canvas/DOM 間でズレる。
- 対策案: ja 用 segmentation データを同梱／ロードするか、line-break 規則を DOM 側と
  揃える（少なくとも禁則・分割可否の整合）。

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

### 問題 1 の根因: emoji コードポイントにグリフ供給が無い

`diagnose_glyph_ink` の出力（`HAYATE_WRITE_SCREENSHOT=1` 必須）:

```
[GLYPH-INK]     0 px  U+1F319 🌙 emoji moon      ← 空白（バグ）
[GLYPH-INK]     0 px  U+1F311 🌑 emoji new moon  ← 空白
[GLYPH-INK]   585 px  U+2600 ☀ sun dingbat       ← 描画される
[GLYPH-INK]   138 px  U+263D ☽ first-quarter      ← 描画される
[GLYPH-INK]   148 px  U+263E ☾ last-quarter       ← 描画される
[GLYPH-INK]   169 px  U+2713 ✓ /  234 px U+2715 ✕ /  284 px ク /  259 px A
```

境界は明確で、**astral plane の emoji (U+1Fxxx) だけグリフが無く**、BMP の dingbat
(U+26xx) はある。根因は Canvas モードのフォント供給:
`Hayate/crates/adapters/web/fonts.json`（builtin フォント manifest）に登録されるのは
**NotoSansJP / Inter / Lato / M PLUS Rounded 1c のみで emoji フォントが無い**。
バンドルされた emoji フォントも無く、WASM 環境では OS の emoji フォントへの fallback も
効かない。よって 🌙 (U+1F319) はどのフォントにもグリフが無く空白になる。DOM モードは
ブラウザが OS emoji フォントへ fallback するため表示され、ここが乖離点。

- 確定修正案 (contained): `App.tsx` のテーマトグル `🌙`(U+1F319) を **☾ (U+263E)**
  もしくは ☽ (U+263D) へ置換。既に動く ☀ (U+2600) と対になり、DOM/Canvas 両方で描画。
- 系統的修正案: emoji フォントを builtin manifest / バンドルへ追加し fallback を通す
  （データ量増。色 emoji の扱いは別途）。

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

- 修正案: parley を `LineSegmenter::new_auto(opt)`（CJK 含む complex データをロード）へ
  変更。ただし vendored 依存の改変かつ icu_segmenter のデータ/バイナリ増を伴うため、
  影響範囲が大きく単独の小修正ではない（要判断）。

### 回帰シグナル

`diagnose_glyph_ink`（ink 数）と `render_glyph_coverage` / `render_tasks_screen`（目視）を
そのまま回帰確認に使える。問題 1 を修正したら `🌙` ではなく採用 dingbat の ink が
0 でないこと、Tasks 画面でトグルにアイコンが出ることを確認する。

## スクリーンショット

![Tasks 画面 (tiny-skia)](./tiny-skia-tasks.png)

![グリフ被覆 (tiny-skia)](./tiny-skia-glyphs.png)
