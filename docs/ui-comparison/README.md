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

## スクリーンショット

![Tasks 画面 (tiny-skia)](./tiny-skia-tasks.png)

![グリフ被覆 (tiny-skia)](./tiny-skia-glyphs.png)
