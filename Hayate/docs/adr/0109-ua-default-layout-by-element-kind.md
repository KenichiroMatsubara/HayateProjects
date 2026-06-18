---
status: accepted
---

# 要素種別ごとの UA 既定レイアウトを core が供給する（button=内容を縦中央 / text-input=フォント感応の既定幅）

**Date: 2026-06-18**

> 本 ADR は決定のみを記録する（実装は後続・issue #390 の診断から）。ADR-0105 が
> element-kind の UA 既定 **cursor** を core 供給と決めたのと同型の判断を、**レイアウト
> 既定**に対して行う。

## Context

CSS gallery の vello↔dom 比較（issue #390）で、Canvas 経路だけ次が崩れた:

- **button**（`Click` / `Hover me` / `Press me` 等、`align` 未指定）の文字が背景ボックス
  上端に貼り付き上端クリップ。DOM では縦中央。
- **text-input**（`inputStyle()`、`width` 未指定）が padding 幅だけ（≒24px）に潰れ、
  placeholder が左端で 1 文字ずつ折り返し、border が左端の縦長スリバーになる。DOM では
  フィールド幅で 1 行。

根因は 2 つ（同一根因ではない）:

- **A**: `text-input` は Taffy projection 上 measure 関数を持たないリーフ → intrinsic
  content width = 0。`width:auto` ＋ flex-grow なし ＋ cross 軸が stretch 以外で padding
  幅まで潰れる。ブラウザ `<input>` は `size`（既定 20）由来の既定幅を UA で持つ。
- **B**: `button` は素の `taffy::Style::default()` に projection される → 子が
  `align-items:stretch` で button 高さまで伸び、グリフが cross 軸 start（上端）に置かれる。
  ブラウザ `<button>` は内容を縦中央に置く（UA）。

いずれも DOM 側はブラウザ UA で既に「正しい見た目」を出しており、欠落は **core（Canvas）
だけ**。これは ADR-0002 の Badge 問題の box/layout 版で、ADR-0002 が除去対象とする「DOM
だけ動いていた偶然」と、ADR-0105 が core 供給と決めた「要素種別の正当な UA セマンティクス」
の境目をどう引くかが問われた。

## Decision

input の既定幅と button の内容センタリングは、除去すべき偶然ではなく **要素種別の正当な
UA セマンティクス**であり、**core が element-kind 既定レイアウトとして供給して両レンダラーを
一致させる**（ADR-0105 と同じ思想）。Canvas 正準（core がセマンティクスを定義する）は保たれ、
値はブラウザ UA を写す。解決順は ADR-0105 と同じ **明示 > element-kind 既定 > Taffy 既定**。

1. **button 既定レイアウト**: cross 軸（縦）= `align-items: center`、main 軸（横）=
   `justify-content: flex-start`（左）。横を中央にしないのは、(i) 症状 2 は縦のクリップで横は
   無関係、(ii) DOM 現状（`BASE_STYLE` の `text-align: inherit`＝左）と一致し新規乖離ゼロ、
   (iii) `justify` 未指定の既存 button（todo 行ラベル等）の左寄せを壊さないため。横中央が
   欲しい button は明示 `justify-content: center` を書く（ブラウザで `text-align:center` を
   書くのと同じ作者責任）。

2. **text-input 既定幅**: text-input に **measure 関数**を与え、明示 `width` が無いとき
   intrinsic content width として **N=20 文字分の幅を現在のフォントで実測して返す**
   （フォント感応・font-size に追従）。`width:auto` のときだけ効き、明示 `width` /
   `flex-grow` / stretch があればそれが優先（Taffy の intrinsic 解決順）。スコープは
   **text-input のみ**（view/button は「中身が無ければ 0 幅」が正しいので触らない）。

3. **実装は core-only**: DOM 側はブラウザ UA で既に一致するため、`element_kinds.json` /
   TS / DOM Renderer は **無改修**。kind ごとの layout 既定は Taffy Style の話で TS 側に対応
   表が存在せず（DOM は CSS/UA に委ねる）、消費者が構造的に居ないので両生成はしない。

## Considered Options

- **ADR-0002 厳格読み（gallery の authoring バグ／DOM 抑制）** — input/button に width/align
  を必ず書かせる、または DOM 側で UA 幅・センタリングを抑制して Canvas 同様に潰す。input が
  0 幅・button が上端クリップは予測可能性の利得ではなく開発者の普遍的期待に反するため却下
  （cursor の ADR-0105 と同じ理由）。
- **button を両軸中央既定**（native `<button>` 寄せ）— `justify` 未指定の既存 button（todo
  ラベル）が横中央化して回帰し、DOM の `text-align:inherit` とも乖離するため却下。縦のみ中央に
  留める。
- **text-input に固定 px の既定幅/flex-basis** — 実装は軽いが font-size に追従せず（大フォント
  で自テキストをクリップ）UA 非忠実なマジックナンバーのため却下。measure（フォント感応）を採る。
- **spec 単一ソース＋両生成（ADR-0105 純化）** — `element_kinds.json` に `defaultJustify` /
  `defaultInputSizeCh` 等を足し DOM も `width:20ch` を明示して px 一致まで狙う。DOM は UA で
  既に一致し、得るのは px 一致のみ（既存テストも実機 Chromium 校正待ちと明言）。ブラウザ自身の
  サイジングと張り合う改修が増えるため、現時点では core-only に留める。

## Consequences

- `text-input` に新しい `MeasureCtx` 分岐（既定幅）を足す。intrinsic 幅は実テキスト内容に
  依存させない（ブラウザ `<input>` は value で伸びず size 幅で固定＋スクロール）。
- button の kind 既定は base `layout_style` に焼き、明示 prop がその上に重なる（「明示 > kind
  既定」が自然成立）。pseudo/variant 由来の明示も同様に優先。
- 横中央が欲しい既存 button は明示 `justify-content: center` を足す必要がある（影響は app/
  gallery の該当 button のみ。todo ラベルのような左寄せは無傷）。
- px 一致は非目標。意味論パリティ（input は妥当な既定幅／button は内容を縦中央）は core 実装
  だけで両レンダラー一致する。

## 関係

- ADR-0105（要素種別 UA 既定 cursor を core 供給）: 本 ADR は同じ思想を **layout 既定**へ拡張。
  ただし cursor は Canvas/DOM 双方に欠落があり spec 単一ソース＋両生成が要ったのに対し、layout は
  DOM が UA で既に一致するため core-only。
- ADR-0002（Hayate CSS セマンティクスはレンダラー非依存・Canvas 正準）: 本件は その box/layout 版。
  「正当な UA セマンティクス」と「DOM だけの偶然」の境を、cursor と同じ基準で前者に置く。
- ADR-0102（Canvas の視覚お手本は DOM）: 既定値がブラウザ UA を写す根拠。
