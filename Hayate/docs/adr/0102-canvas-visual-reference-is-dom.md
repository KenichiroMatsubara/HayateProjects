# Canvas Mode の視覚的お手本は DOM（Chromium ブラウザ）とする

**Status: accepted**

**Date: 2026-06-16**

## Context

`docs/ui-comparison` の操作中比較で、Canvas Mode（Vello / tiny-skia）と DOM/HTML 系で
text-input の chrome — placeholder 色・focus 表示・IME preedit 装飾・選択ハイライト —
の見た目が乖離することが分かった（findings #4–#7）。

どちらの見た目を正準とするかが未規定だった。**意味論パリティ**（CONTEXT）は継承・
scroll chaining・font 合成といった**挙動**の契約で、chrome の**見た目**の正準は定めて
いない。**Selection Chrome**（CONTEXT）は「Canvas は core が Material / Cupertino テーマで
一度描画し、DOM 経路は browser native に委ねる」と非 look-parity を明文化していたが、
placeholder / focus / preedit の見た目は未規定で、Canvas が browser 既定から外れていた
（例：placeholder を本文色で描く、preedit に下線が無い、`:focus` の 1px ボーダーが薄い）。

同一アプリが Canvas Mode と HTML Mode の二通りで描かれる（`hayate-adapter-web`、
EditContext 有無で切替・ADR-0016 / ADR-0048）以上、両モードの見た目が食い違うのは
ユーザー体験上の不具合であり、どちらかを正準のお手本に定める必要がある。

## Decision

**Canvas Mode の視覚的お手本は DOM モード（ブラウザ既定描画）とする。**

- 同一コードベースでお手本になり得る視覚実体は DOM 経路だけなので、限りなく DOM が
  基本のお手本。ブラウザが描く chrome は Canvas も**ブラウザ既定に寄せる**。
- Canvas Mode は EditContext 必須で実質 Chromium 限定（ADR-0016）。よってお手本
  ブラウザは **Chromium** に確定し、「ブラウザ既定に寄せる」が決定的になる。
- **ブラウザに無い概念に限り** Android-native（ADR-0087）をお手本にする — モバイル
  選択の drag handle / フローティングツールバー / 拡大鏡など。

各 chrome への適用:

- **placeholder**：Chromium UA `::placeholder`（muted）を再現する。authorable にしない
  （DOM 経路も browser 既定のまま）。
- **focus**：HTML Mode が text-input に撒いていた `outline: none`（`html.rs`）は本原則
  違反の**バグであり除去**する（focus 表示は browser の `:focus-visible` に委ねる）。
  Canvas は native focus ring を **`:focus-visible` 忠実**に再現する。
- **IME preedit**：clause 分割下線を Chromium 同様に再現する（EditContext
  `textformatupdate` を JS→wire→core に配管し、preedit を範囲付きに拡張）。
- **selection**：ハイライト tint は browser `::selection` に寄せる。handle / ツールバー /
  拡大鏡のみ Android-native。

## Consequences

- **Selection Chrome（CONTEXT）を改定**：「Material / Cupertino テーマ」は browser に
  無い handle / toolbar / 拡大鏡に限定し、highlight tint は browser 寄せにする。
- core に**入力モダリティ追跡**（pointer / keyboard）を追加する（`:focus-visible` 用）。
- `EditState::preedit` を `Option<String>` から**範囲付き**へ拡張し、wire に
  `textformatupdate` surface を足す。
- placeholder 色・focus ring の色 / 太さ・`::selection` 色などの**実値は Chromium 実描画で
  校正**する（本 ADR は方針のみ）。
- HTML Mode の input 正規化（`background` / `font` / `color: inherit`）は維持し、
  `outline: none` のみ除去する。
- 「意味論パリティ」（挙動の契約）とは直交。本 ADR は見た目の正準を定めるもの。

## Considered Options

- **Canvas を独自 Material / Cupertino テーマで描き browser と独立（却下）**：同一アプリの
  2 モードで見た目が恒常的に乖離し、お手本が存在しなくなる。
- **Hayate CSS に `::placeholder` / focus-ring / composition トークンを新設し、DOM を
  曲げて両者を新しい正準へ揃える（却下）**：CSS サーフェスが増え、native input の既定
  挙動と戦い続ける。「DOM をお手本にする」というシンプルさを失う。

## 関係

- ADR-0016（Canvas Mode は EditContext 専用＝実質 Chromium 限定）を、視覚お手本を
  Chromium に確定する根拠に用いる。
- ADR-0029（HTML Mode はブラウザ CSS レイアウト）／ADR-0048（tiny-skia CPU
  フォールバック）の上で、HTML / Canvas 両モードの見た目正準を browser に定める。
- ADR-0087（Android が first native target）を、browser に無い概念のお手本に用いる。
- CONTEXT「Selection Chrome」を tint=browser 寄せに改定する。
