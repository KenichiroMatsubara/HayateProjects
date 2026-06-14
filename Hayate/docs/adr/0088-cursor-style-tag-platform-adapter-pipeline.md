# cursor は style_tags に追加し OS カーソル変更は Platform Adapter が担う

**Status: accepted**

**Date: 2026-06-13**

## Context

ボタンやリンク相当の要素では `cursor: pointer`、テキスト入力では `cursor: text`、ドラッグ中は `cursor: grabbing` など、インタラクションに応じてポインタ形状を変えることは基本的な DX 要件である。この機能を誰が担うかの設計を決める必要がある。

候補として (a) style_tags に追加して Hayate コアがカーソル解決を担う、(b) Tsubame（Framework 層）が要素の `onPointerEnter`/`onPointerLeave` に応じて DOM の `style.cursor` を直接書き換える、の2案があった。

## Decision

`cursor` を `style_tags.json` に追加する。Tsubame は OS/ブラウザカーソルを触らない。

- Hayate コアはポインタ移動時に Hit-test で「カーソル下の最前面要素」を解決し、その `cursor` 値を `on_pointer_move` の出力値として Platform Adapter に返す。
- Platform Adapter（例: `hayate-adapter-web`）がブラウザの `document.body.style.cursor` を設定する。
- Tsubame/Framework 層は cursor の解決にも設定にも関与しない。

(b) を採用しない理由: `onPointerEnter`/`onPointerLeave` は要素単位のイベントであり、子要素が DOM にない Canvas Renderer ではイベントが存在しない。Platform Adapter が唯一カーソル設定 API（ブラウザ CSS / Android `PointerIcon` 等）を知っているため、そこに集約するのが Renderer 非依存（ADR-0002）の原則と整合する。

## Consequences

- `cursor` の宣言だけでなく、`on_pointer_move` の戻り値型を拡張して解決済みカーソル値を運ぶパイプラインが必要になる。
- Platform Adapter ごとに cursor 設定の実装が必要（Web: `style.cursor`、Android: `PointerIcon` など）。
- Canvas Renderer と DOM Renderer のどちらでも同一の `cursor` 宣言が機能する。
