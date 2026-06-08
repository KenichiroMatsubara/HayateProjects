# tag→CSS DOM 写像を spec の単一正本にする（ADR-0055 を DOM 写像まで拡張、候補 B2）

**Status: accepted（ADR-0049/0055 の単一正本 scope を DOM 写像まで拡張）**

**Date: 2026-06-07**

## Context

`style_tags.json`（spec 正本）は wire（`name` / `value` / `encodeFrom` / `params`）を所有するが、**tag→CSS の DOM 写像**（CSS プロパティ名・値フォーマット・`DOM_EXTRAS` 副作用）を持たない。その知識は **2 generator に二分**されている：

- `dom_style_mapper.rs`（Rust・生成、Hayate HTML Mode 用）：`BackgroundColor→"background-color"` rgba、`BorderRadius→"border-radius"` px… を Rust generator（`lib.rs`）が**ハンドコード**。
- `catalog.ts`（TS・生成、Tsubame DOM Renderer 用）：同写像 ＋ `DOM_EXTRAS`（`borderWidth→borderStyle:solid` 等）が **`gen-catalog.mjs` の config にだけ**存在。

新 tag の DOM 写像追加は **spec ＋ Rust generator ＋ TS generator config の3箇所**。2つの独立実装は**ドリフトしうる**。ドリフトすると **Hayate HTML Mode と Tsubame DOM Renderer が乖離**し、web 開発の **Canvas↔DOM デザイン比較**（ADR-0012 の dev-velocity ツール）が信用できなくなる。

## Decision

**spec の単一正本 scope（ADR-0049/0055）を tag→CSS DOM 写像まで拡張する。** `style_tags.json` の各 entry に `domCss` を追加し、`dom_style_mapper.rs`（Rust）と `catalog.ts`（TS）を spec から生成する。

```json
{ "name": "BORDER_WIDTH", "...": "...",
  "domCss": { "property": "border-width", "format": "px",
    "extras": [{ "property": "border-style", "whenPositive": "solid", "whenZero": "none" }] } }
```

- `DOM_EXTRAS` を `gen-catalog.mjs` config から **spec へ移す**。
- DOM 写像の無い/特殊な tag は `domCss: null`。
- placement は `style_tags.json` の `domCss` フィールド（tag と co-located・1ファイル。独立 `dom_css.json` は不採用＝単一性が見えにくい）。

## Consequences

- tag→CSS 知識が **spec 1箇所**に集約。3箇所更新 → 1箇所。
- 2 generator が同一 spec 解釈に従い、**Hayate HTML Mode と Tsubame DOM Renderer が構造的に一致** → web Canvas↔DOM 比較が信用できる。
- Rust generator のハンドコード dispatch（`lib.rs`）と TS の `DOM_EXTRAS` config を撤去。
- DOM 写像は web 専用（HTML Mode・Tsubame DOM）で native は使わないが、単一化は dev-comparison の信頼性を担保する目的（native 本体・ADR-0012 と矛盾しない）。

## Considered Options

- **per-generator 維持（現状）**：ドリフト risk・3箇所更新。却下。
- **独立 `dom_css.json`**：単一正本だが tag 情報が分割。tag 直付け（`domCss`）を採用。
- **spec が `domCss` を所有・両側生成（本決定）**：ADR-0055 の延長。

## 関係

- ADR-0049：プロトコル定数の単一正本。
- ADR-0055：単一正本を wire codec まで拡張 — 本 ADR が DOM 写像まで further 拡張。
- ADR-0029：HTML Mode はブラウザ CSS。DOM 写像はその実体。
- ADR-0012：web Canvas↔DOM 比較は dev-velocity ツール。DOM 写像の一致がその前提。
