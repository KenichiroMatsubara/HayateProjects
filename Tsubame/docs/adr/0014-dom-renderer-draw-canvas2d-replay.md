# DOM Renderer の draw は同一 painter を `<canvas>` 2D へ replay する（SVG 生成を棄却）

**Status: accepted**

**Date: 2026-07-06**

## Context

draw（Hayate ADR-0141）は意味論パリティ対象——同じ painter が Hayate Renderer（GPU Canvas）と DOM Renderer で同じ絵を出す必要がある。painter は JS 関数なので、DOM Renderer では wire を通さず直接実行できる。実現形は 2 つ: `<canvas>` 2D コンテキストへの replay と、SVG（`<svg><path d="...">`）の生成。

## Decision

**`draw` 付き view に `<canvas>` を敷き、painter の記録を CanvasRenderingContext2D 呼び出しへ写す（replay）。** DPR 追従（物理解像度 = 論理サイズ × devicePixelRatio、painter からは不可視）は DOM Renderer の責務。`overflow: visible`（既定）のはみ出しには canvas を box より大きく確保する（余白量は名前付き定数）。再描画はクリアして全 replay（差分管理はしない）。

## Rationale

決定打は最終目標が「Flutter Canvas 同等の表現力＝グラデーション〜シェーダまで」であること（PRD #723 の階層 4〜7）。

- **blend / saveLayer**: 2D は `globalCompositeOperation` + レイヤで概ね写せる。SVG の blend は CSS `mix-blend-mode` 頼みで意味論の食い違いが大きい。
- **drawImage / テキスト**: 2D はネイティブ。SVG は要素化でレイアウト・フォント経路が絡み複雑化。
- **フィルタ / シェーダ**: 2D は `filter` + 将来 WebGPU 併用の道がある。SVG フィルタは別言語（feGaussianBlur 等）への翻訳でパリティ検証が破綻する。
- **再描画モデル**: replay は「クリアして再実行」で終わる。SVG は前回生成 DOM との差分管理が必要になり、DOM Renderer に第 2 の reconciler を飼うことになる。

## Considered Options

- **SVG 生成**: 階層 1〜3 なら書けるが上記の通り階層 4 以降で行き詰まる。DOM 検査ツールで絵が見える・DPR がブラウザ任せという利点はあるが、a11y は将来の明示 label property で扱う話であり決定打にならない。却下。

## Consequences

- `<canvas>` はラスターなので DOM 検査ツールで絵の中身は見えない（Canvas 経路の全 UI 1 canvas と同条件）。
- 「visible + 大きくはみ出す絵」は canvas の余白確保コストが上がる。余白定数は名前付きで、必要なら後から人力チューニング可能にしておく。
