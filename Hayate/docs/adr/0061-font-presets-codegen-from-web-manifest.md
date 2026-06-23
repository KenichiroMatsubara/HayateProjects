# Web フォントプリセットを web adapter の fonts.json から codegen する（app font は文字列名で接続）

**Status: accepted**

**Date: 2026-06-07**

決定: [decisions-pending Open #1](../decisions-pending.md) を解決する。

## Context

`font_family` の正本化が未決だった。decisions-pending Open #1 は「spec プリセット `font_family` と `hayate.config` 由来の app font をどう接続するか。必要なら `100+` を app font 用予約帯に」としていた。

しかし実態を確認すると前提が崩れている:

- `enums.json` の `font_family` は **`string_values: true`**（値は `"Noto Sans JP"` 等の**文字列**、数値コードではない）。
- `style_tags.json` の `FONT_FAMILY` は `encodeFrom: font-family` で **wire 上は文字列**（`tag + len + bytes`）。
- `register_font(family_name: &str, bytes)` も**文字列名でキー**。`configure_fonts`（ADR-0044）が app font を名前で登録する。
- 解決は `build_text_layout` が `"{family}, {DEFAULT}"` の CSS スタックを組み、Parley が登録済みフォントを使い、無ければ `FetchFont` で取得、それも無ければ bundled default にフォールバックする。

つまり **font_family は端から端まで文字列**で、app font は名前だけで既に動く。**数値 app font ID も「100+ 予約帯」も不要（前提が数値 enum だった旧案であり obsolete）。**

残る実問題は、プリセットの二重手書きである: `enums.json` の `font_family` 名（platform 非依存）と、web adapter `element_renderer.rs` の手書き `builtin_font_url`（名前→CDN URL）が別管理で drift しうる。

ADR-0043 は「名前→CDN URL は web adapter 所有（platform 固有。native は OS から解決）」と決めており、URL を spec/core に入れることは却下済み。

## Decision

### 1. 数値 app font ID を採らない（「100+ 予約帯」を reject）

`font_family` は文字列を正とする。app font は `hayate.config`（ADR-0044）で `{family, url}` を宣言し、`register_font` に**名前**で登録して参照する。wire 上の数値 ID は導入しない。

### 2. プリセット名の正本は spec、URL は web adapter（α）

- **spec `proto/spec/enums.json` の `font_family`** = プリセット「名前」の正本（platform 非依存。cross-adapter で同一名＝同一 fallback、ADR-0012 等階級）。
- **web adapter の font マニフェスト `Hayate/crates/platform/web/fonts.json`** = `[{ family, url, scripts? }]`（web 固有の調達情報）。これから **`builtin_font_url` を codegen**（手書き match を廃止）。
- **URL は web adapter 層に留める**（ADR-0043 維持）。native adapter は将来 自身の manifest（OS フォント名）を持つ。
- **coverage check**: `fonts.json` は `enums.json` の全プリセット名に URL を与える。欠けは check エラー。プリセットは cross-adapter 契約なので、まず spec に名前を足してから各 adapter が調達手段を与える。

### 3. 解決順序と検証（現挙動の明文化）

- 優先順位: **app 登録フォント（`configure_fonts`/`register_font`）> builtin プリセット fetch（`FetchFont`→`builtin_font_url`）> bundled default**。同名なら app 登録が builtin を shadow する（local 優先・オフライン堅牢）。
- 未知ファミリ（未登録かつ非プリセット）は **silent に default へフォールバック**（CSS 流。エラーにしない）。

### 4. TS 型

`StylePatch.fontFamily` は開いた `string` を維持（app 名を阻害しない）。任意で spec から `PresetFontFamily` union を生成し `PresetFontFamily | (string & {})` として autocomplete を足してよい（DX のみ、必須ではない）。

## Considered Options

- **β: spec に `fonts.json`（URL 込み）1 枚を置き全生成**: 最も単純な単一正本だが、CDN URL が platform 非依存 spec に入り ADR-0043 と矛盾。native が url 欄を無視する運用となり層が濁る。却下。
- **「100+ 予約帯」で数値 app font ID**: `font_family` が文字列の現設計では無意味。却下。
- **二重手書きリスト容認**: drift リスク残置。却下。

## Consequences

- web adapter の手書き `builtin_font_url` が `fonts.json` からの生成物に置き換わる。
- `enums.json` プリセット名と `fonts.json` の coverage が CI で検証される。
- app font は数値 ID 不要・名前のみで接続。decisions-pending Open #1 を closed とする。
- 関連: ADR-0042（codepoint→family は core 所有・本 ADR の対象外）、ADR-0043（URL dispatch は adapter）、ADR-0044（app font config）、ADR-0012（等階級）。

## Implementation Tasks（完了）

1. ✅ `Hayate/crates/platform/web/fonts.json` 新設 — 全プリセット family→url を移植。
2. ✅ `build.rs` codegen — `OUT_DIR/builtin_fonts_gen.rs` を生成、`builtin_fonts.rs` が include。
3. ✅ coverage check — `builtin_fonts.rs` テストで `enums.json` 全 preset を検証。
4. ⬜ （任意）TS `PresetFontFamily` autocomplete。
5. ✅ 回帰 — `builtin_font_url("Noto Sans JP")` 等の URL 固定テスト。
