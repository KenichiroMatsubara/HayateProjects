# Canvas ペインターの欠落グリフ・プレースホルダー描画

**Status: accepted**

**Date: 2026-06**

Canvas モードのシーンペインター（vello / tiny-skia）は、シェーピング済みの
`RenderGlyph`（グリフ id・座標のみ）を受け取って描画する。プライマリフォントに
存在しないコードポイント（例: `✕` U+2715, `✗` U+2717, 多くの絵文字）はシェーピングで
グリフ id 0（`.notdef`）になる。バンドルフォント `NotoSansJP.ttf` の `.notdef` は
アウトラインを持たないため、これまで欠落グリフは **無言の空白**（0 ink）に潰れていた。
一方 DOM レンダラーはブラウザのシステムフォントフォールバックで救済される（issue #427）。

ADR-0042 は CJK ブロックに対する `.notdef` 検出＋動的ダウンロードを定義済みだが、
これは CJK スクリプトに限定され、記号・絵文字などブロック外のコードポイントには働かない。
ペインターはコードポイントもフォールバック用バイト列も持たず、解決済みグリフ id しか
見えないため、ペインター層で「実フォントによる再シェーピング」を行うことはできない。

## 決定

ペインター層では **意図的に見えるプレースホルダー箱** を描く。`.notdef`（グリフ id 0）を
検出したら、フォントの無言の箱を描く代わりに、テキスト色で中空の矩形を 1 つ描画する。
ロジックは `hayate_core::render::missing_glyph` に集約し、vello と tiny-skia の両ペインターが
共有する：

- `NOTDEF_GLYPH_ID: u32 = 0` ── 全 OpenType/TrueType フォント共通の `.notdef` id
- `is_notdef(&RenderGlyph) -> bool` ── 判定ヘルパー
- `missing_glyph_placeholder(&RenderGlyph, font_size) -> MissingGlyphPlaceholder`
  ── ベースライン上のキャップハイト帯に置くプレースホルダー箱の幾何（run-local 座標）を返す。
  `RenderGlyph` は advance を持たないため、箱の寸法は em（`font_size`）から算出する。
- `FALLBACK_FONT_CHAIN: &[&str]` ── 将来フォールバックフォント鎖を実フォントへ接続する
  ための **名前付き定数（足場）**。現状はプレースホルダーのファミリ名一覧であり、リストの
  確定と実フォント資産の配線は issue #427 の後続作業。

両ペインターは描画ループで `is_notdef` を見て分岐し、tiny-skia は矩形パスを stroke、
vello は実グリフ描画パスから `.notdef` を除外したうえで矩形を stroke する。データ構造
（`TextRunData` / `RenderGlyph`）と新規フォント資産には一切手を入れない。

## 却下した代替案

- **フォールバック鎖に実フォントを同梱し再シェーピングする**: 新規フォント資産・
  `TextRunData`/`RenderGlyph` の拡張・共通リゾルバでの `.notdef` クラスタ再シェーピングが
  必要で、最小修正の範囲を大きく超える。issue #427 は「プレースホルダーで可・確定は後続」と
  明記しており、まずは可視化を最小コストで達成する本案を採る（鎖の確定は後続）。
- **シェーピング層でフォールバックを解決する**: Parley のフォントスタックに鎖を積めば
  クラスタ単位フォールバックが決定的になるが、これはペインターではなくコアのシェーピング層の
  変更であり、issue がペインターを対象としている範囲から外れる。
- **何もしない（DOM フォールバックに委ねる）**: Canvas モードが主ターゲットであり、
  欠落グリフが無言で消える現状を放置することになる。却下。

## 影響

- `crates/core/src/render/missing_glyph.rs`: 新規モジュール。定数・ヘルパー・幾何計算と単体テスト
- `crates/core/src/render/mod.rs`・`lib.rs`: 上記の re-export
- `crates/scene-renderers/tiny-skia/src/painter.rs`: `draw_text_run` で `.notdef` 分岐、
  `draw_missing_glyph` ヘルパー追加
- `crates/scene-renderers/vello/src/painter.rs`: `draw_text_run` で `.notdef` を実グリフ描画から
  除外し、プレースホルダー矩形を stroke
- `crates/scene-renderers/tiny-skia/tests/app_screenshot.rs`: U+2715 が無言の箱ではなく
  可視プレースホルダーを描くことのリグレッションテスト
