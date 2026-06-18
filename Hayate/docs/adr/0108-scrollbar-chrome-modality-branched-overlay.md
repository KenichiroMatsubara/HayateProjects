# Scrollbar Chrome は Pointer Modality 分岐の overlay とする

**Status: accepted**

**Date: 2026-06-18**

## Context

`docs/ui-comparison` の vello ↔ DOM 比較（issue #391）で、`scroll-view` のスクロールバーが
DOM Renderer（`overflow: auto` の native UA chrome）では見えるのに Canvas（vello / tiny-skia）
では描かれないことが分かった。内容の clip 自体は出ている。

当初は「意味論パリティは挙動の契約で chrome の画素一致は対象外（#371）」を根拠に *by-design・
非目標* と整理しかけた。しかし ADR-0102 は **Canvas Mode の視覚的お手本は DOM** と定め、
*「ブラウザが描く chrome は Canvas もブラウザ既定に寄せる」* を原則化している。スクロールバーは
「DOM が描き Canvas が描いていない chrome」そのもので、これは 0102 が修正前の focus ring と
同じバケツ — **0102 が及ぶ既知の未実装ギャップ**であって、確定した非目標ではない。

加えてスクロールバーは 0102 が列挙する受動 chrome（placeholder / focus ring / preedit 下線 /
selection tint）と違い、**操作可能なコントロール**（thumb ドラッグ・track クリック）でありうる。
さらに**モバイルには常駐スクロールバーが無い** — Android / iOS はスクロール中だけ出てフェードする
**非操作の transient indicator** で、内容フリックで動かす。お手本が単一でない。

## Decision

**Scrollbar Chrome を core が描く chrome とし、Pointer Modality（PointerKind）で形態を分岐する。
全 modality で overlay（レイアウト空間を予約しない）とする。**

- **Mouse / Pen**：Chromium（ADR-0102）をお手本に、内容の上へ overlay 描画する **操作可能** な
  スクロールバー。thumb ドラッグ・track クリックで Scroll Offset を動かす。
- **Touch**：Android-native（ADR-0087 / ADR-0102 の「ブラウザに無い形態は Android をお手本」）を
  お手本に、スクロール中のみ出る **非操作の transient indicator**。thumb ドラッグの概念を持たない。
- **overlay 固定（レイアウト非予約）**：いずれの形態も content box 幅を食わない。よって Taffy に
  scrollbar gutter 予約を実装しない。意味論パリティ（content box 幅はレンダラー間で一致せねば
  ならない）の帰結として、**DOM 経路も overlay に固定**する（gutter を予約しない）。
- **操作意味論は Scroll Offset に収斂**：thumb ドラッグ→`element_set_scroll_offset` は、wheel 積算
  （ADR-0046）・chaining（ADR-0084）と同じく **Scroll Offset レベルでパリティ**する。DOM は native
  ドラッグが同じ Scroll Offset を生む。thumb 幾何は core が既に持つ Scroll Offset と content size
  （`element_content_size`）から算出し、ScrollView anchor 配下に overlay として lowering する。

これは ADR-0104（modality 依存の選択 chrome ライフサイクル）と同軸・同型であり、Scrollbar Chrome を
Selection Chrome の姉妹概念として CONTEXT に置く。

## Consequences

- **#391 は by-design・クローズではない**：ADR-0102 が及ぶ既知ギャップとして整理し直し、実装は
  follow-up issue に切る。
- **chaining 意味論（ADR-0084）とは直交**：本 ADR は scrollbar の*見た目と操作*。残デルタ伝播の
  意味論パリティは既に成立しており本 ADR で変えない。
- DOM Renderer の `scroll-view`（現 `overflow: auto`）を **overlay 固定**へ寄せる小改修が要る
  （gallery の DOM ベースラインが classic 予約 → overlay に変わる）。
- §7 Scroll に thumb ドラッグ／track クリックの操作意味論を追補する（実装は follow-up）。
- 実値（scrollbar 幅・thumb 色 / 不透明度・フェード時間・indicator 形状）は ADR-0102 同様
  **Chromium / Android 実描画で校正**する。本 ADR は形態方針のみ。

## Considered Options

- **by-design として描かない（却下）**：ADR-0102 の「browser が描く chrome は Canvas も寄せる」に
  反する。focus ring と同じ未実装ギャップを非目標と誤整理することになる。
- **classic（gutter 予約）スクロールバー（却下）**：Linux/Windows デスクトップ Chromium に最も忠実
  だが、Canvas 側に Taffy の scrollbar gutter 予約（§3 レイアウト波及・content box 縮小）を作り込む
  必要があり重い。overlay 化で DOM ベースラインを寄せる方が遥かに小さい。
- **modality 非依存の単一形態（却下）**：実装は単純だが、Touch で「掴めるはずのバー」を出すのは
  Android UX 乖離で 0102/0087 のお手本方針に反する。
- **paint-only（操作なし）でまず 0102 を満たす（却下）**：0102 の最小は満たすが、ドラッグできない
  dead な bar は DOM と挙動が乖離し中途半端。操作込みで一体スコープとする。

## 関係

- ADR-0102（Canvas 視覚お手本＝DOM/Chromium、ブラウザに無い形態は Android）を継承し、scrollbar に
  適用。
- ADR-0104（Pointer Modality 依存の chrome ライフサイクル）と同軸 — Scrollbar Chrome も
  PointerKind で分岐。
- ADR-0046（Scroll Offset は core 所有）/ ADR-0084（scroll chaining）の Scroll Offset シームに
  thumb ドラッグを載せる。挙動の意味論統一は system-wide ADR-0002 に従う。
- ADR-0087（Android が first native target）を Touch indicator のお手本に用いる。
- CONTEXT「Scrollbar Chrome」を新設、「Selection Chrome」の姉妹として置く。
