# element property は閉じた typed 語彙とし、未知 property はエラーにする（候補 TSUB-02）

**Status: accepted（ELEM-01/ADR-0009 の閉じた語彙原則を property に適用。ADR-0059 の reject パターンを踏襲）**

**Date: 2026-06-07**

## Context

`IRenderer.setProperty(id, name: string, value: unknown)` は**型なしのエスケープハッチ**。DOM Renderer（`dom-renderer.ts:126`）は4つの意味プロパティ（`value` / `placeholder` / `disabled` / `src`）を typed に処理し、**残りは `setAttribute(name, value)` にフォールバック**（`:159–167`）。Canvas Renderer（`canvas-renderer.ts:99`）は**全部 no-op** → 意味プロパティが silent drop（TSUB-02 の実害）。

任意 `setAttribute` フォールバックが意味するのは、**アプリが Hayate の語彙に無い未知プロパティ（`aria-*` / `data-*` / `id` / `class` / 独自 attr）を投げてきている**こと。これは HTML 語彙のリークであり、Hayate がタグで `div`/`span` を禁じている（ELEM-01 / ADR-0009）のと同じクラスの問題：

- `id` / `class`：Hayate に selector / cascade が無い（Hayate CSS）→ 無意味。
- `data-*` / 任意 attr：DOM 固有。closed な element モデルに不要。
- `aria-*`：アクセシビリティは既に first-class（`element_set_aria_label` / `set_role` ＋ AccessKit）。

警告では語彙リークを許してしまう。**タグ禁止と同じくエラーで禁止すべき**。

## Decision

**`setProperty` の untyped エスケープハッチを廃止し、element property を閉じた typed 語彙にする。**

- **既知の意味プロパティを first-class 化**：`value`（text-input 内容＝`text_content`/`EditState`）/ `placeholder`（TextInput の `el.text`）/ `src`（`element_set_src`）/ `disabled`。両 renderer が明示実装し、Canvas は packet mutation へ enqueue（silent drop 消滅）。`aria-label` / `role` は既存 Hayate first-class へ振る。
- **未知 property 名はエラー（禁止）**：silent でも warning でもなく、タグ禁止と同じ扱い。可能なら **build-time**（solid `.tsx` の JSX prop 型で未知 prop を型エラー）＋ **runtime throw（dev）**。既存 `REJECTED_EVENT_PROPS`（`renderer.ts:97` が `onHoverEnter` 等を throw）と同パターン。
- DOM Renderer の `default→setAttribute` フォールバックを**撤去**。
- `disabled`：Hayate に小さな disabled state を新設（interaction イベント抑止＋任意で `:disabled` 擬似スタイル）。

## Consequences

- property が element-kind（ELEM-01）・Hayate CSS と同じく**閉じた語彙**になり、HTML 属性リークが構造的に不能。
- Canvas の silent drop クラスが消滅：意味プロパティは両対応、未知 property はエラー。
- DOM Renderer の任意 `setAttribute` 経路を撤去。
- `aria` は first-class 経由のみ（`setProperty` で `aria-*` を投げる経路を廃止）。
- `disabled` state を新設（小）。`:disabled` は将来の擬似スタイル拡張点。
- solid adapter の `setProperty` フォールバック（`renderer.ts:117`）は、既知名を typed 経路へ・未知名を throw に分岐。

## Considered Options

- **Canvas が任意 attr を dev 警告**：語彙リークを許す。却下（エラーで禁止が closed 語彙原則に整合）。
- **minimal（Canvas で既知名のみ route、untyped channel 維持）**：HTML リーク＋未知の黙認が残る。却下。
- **閉じた語彙＋未知エラー（本決定）**：property を element-kind と同格の closed 語彙に。

## 関係

- ELEM-01 / ADR-0009：閉じた RN 語彙（HTML タグ禁止）を property にも適用。
- ADR-0059：Tsubame Adapter が prop を throw で reject する既存パターンを踏襲。
- ADR-0069（D1）：`value` は `EditState`/`text_content` へ。
- ADR-0058：`placeholder` は TextInput の `el.text`。
- PLAT-03/04（AccessKit）：`aria` は first-class 経由。
