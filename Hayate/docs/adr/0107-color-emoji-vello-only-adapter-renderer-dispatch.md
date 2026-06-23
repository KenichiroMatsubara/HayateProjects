# カラー絵文字は Vello 限定・レンダラ別フォント出し分けはアダプタが持つ

**Status: accepted**

**Date: 2026-06-17**

## Context

ADR-0101 は `.notdef` 検出時のフォールバックフォントを **モノクロ `Noto Emoji`**
に固定した。理由は CPU フォールバックの tiny-skia painter が `outline_glyphs()`
（glyf/CFF アウトライン）しか描けず、COLR/CBDT のカラー絵文字を描画できないためで、
両レンダラ共通の routing（core の `font_coverage`、ADR-0042/0101）が単一ファミリを
返す設計上「最小公倍数 = モノクロ」を選んでいた。

結果として WebGPU primary の **Vello 経路でも emoji がモノクロ**になっていた。だが
Vello（vendored 0.9）は `draw_glyphs().draw()` が COLR/CPAL・bitmap strikes を検出して
`try_draw_colr` に分岐するため、**カラー絵文字フォントを渡せばそのまま色付きで描ける**
（`crates/vendor/vello/src/scene.rs`）。ADR-0101 はこの「Vello 限定でカラー化する」作業を
adapter 層（ADR-0043）の責務として issue #332 に分離していた。

## 決定

### 1. カラー絵文字は Vello 経路に限る（tiny-skia はモノクロ据え置き）

`SceneRendererKind::paints_color_glyphs()` を真とするのは Vello のみ。tiny-skia・
recording・null は偽。COLR/CBDT を tiny-skia で描くには painter の大改修が要り仕様的に
重いため、当面非対応とする。**CPU フォールバック時はモノクロに縮退する**。

### 2. レンダラ別の family→URL 出し分けは web アダプタが持つ（core は不変）

core の codepoint→family（ADR-0042/0101）は **触らない**。emoji の `.notdef` は従来どおり
モノクロ family `Noto Emoji` にルートされる。出し分けは「family→source = アダプタ所有」
（ADR-0043）の延長として web アダプタに閉じる：

- `fonts.json` に `Noto Emoji`(mono) と `Noto Color Emoji`(COLR/CPAL) を併載する。
- `font_url_for_renderer(family, renderer)` が、family が `Noto Emoji` かつアクティブな
  レンダラが `paints_color_glyphs()` のとき **カラービルドの URL** を返す。それ以外の
  family は `builtin_font_url` と同一（レンダラ非依存）。
- 取得したバイト列は **core が要求した family（`Noto Emoji`）の名前で登録**する。よって
  この出し分けは core のルーティングから完全に不可視で、ADR-0101 の coverage 表を汚さない。

`poll_events` の `FetchFont` インターセプト（ADR-0043）が `self.backend.kind()` を渡して
この関数を呼ぶ。

### 3. カラービルドは `Noto Color Emoji-Regular.ttf`（COLRv1 + CPAL）

google/fonts `ofl/notocoloremoji/NotoColorEmoji-Regular.ttf`。COLR・CPAL・glyf・SVG を持つ
COLRv1 ビルドで、fontique/skrifa が解釈でき Vello の `try_draw_colr` 経路に乗る（CBDT
ビットマップ版ではない）。

## 却下した代替案

- **core の routing をレンダラ別にする**：codepoint→family は platform/レンダラ非依存と
  いう ADR-0042 の層分けを壊す。調達層の都合を core に漏らすことになる。→ 不採用。
- **tiny-skia に COLR/CBDT 対応を入れる**：painter の大規模対応が必要で仕様的に重い。
  CPU フォールバックの縮退（モノクロ）は許容できる。→ 本 issue ではスコープ外。
- **常にカラービルドを使う**：tiny-skia ではカラービルドの COLR を無視して glyf 縮退
  （あるいは空白）になり、しかもカラー絵文字フォントは巨大（~24MB）。CPU 経路に重い
  ダウンロードを強いる意味がない。→ 不採用。

## 影響

- `platform/web/fonts.json`：`Noto Color Emoji` エントリを追加（build.rs が
  `builtin_font_url` に取り込む）。
- `platform/web/renderer_selection.rs`：`SceneRendererKind::paints_color_glyphs()` を追加。
- `platform/web/builtin_fonts.rs`：`font_url_for_renderer(family, renderer)` を追加。
  既存 `every_coverage_family_is_procurable` 等は不変（カラー family は coverage 表には
  載せない）。
- `platform/web/canvas.rs`：`poll_events` の `FetchFont` 処理が `font_url_for_renderer`
  に `self.backend.kind()` を渡す。
- 視覚 e2e：小さな COLRv1 テストフォント（`test-support/assets/colr_test_glyphs.ttf`、
  Apache-2.0、provenance 記載）で Vello がカラー描画する（`vello/tests/color_emoji.rs`、
  複数色相を確認）／ tiny-skia は同グリフをモノクロ縮退する
  （`tiny-skia/tests/color_emoji_monochrome.rs`）ことを検証。wgpu アダプタが無い環境では
  Vello 側はスキップ（既存の Vello 視覚テストと同様）。
- 関連：ADR-0042（検出点と層）、ADR-0043（family→URL はアダプタ）、ADR-0101（データ駆動
  coverage 表・モノクロ補完）、ADR-0050（レンダラ選択・フォールバック）。
