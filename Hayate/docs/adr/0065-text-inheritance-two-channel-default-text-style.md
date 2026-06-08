# テキスト継承は text-local ＋ ambient Default Text Style の2チャネルにする（ADR-0047 を supersede）

**Status: accepted（ADR-0047 を supersede。ADR-0063 の inline cascade と整合）**

**Date: 2026-06-07**

## Context

ADR-0047 は「text 系スタイル（`color`/`font-size`/`font-family`）を全 element 横断で top-down に継承する」とし、これを「Flutter モデル」と呼んだ。しかしこれは誤記で、**実物の Flutter は全 widget 横断の継承をしない** — `Container`/View 相当は text スタイルを持たず、block を貫通する text 継承は `DefaultTextStyle`（ambient な既定値専用 InheritedWidget）**のみ**。`Text`/`TextSpan` の通常スタイルは text ローカルで、`DefaultTextStyle` に merge される。ADR-0047 が実装したのは CSS 寄りの「全要素で font プロパティが継承」モデルだった。

これは ADR-0063（2b・IFC）で問題になる。CSS 式だと layout 用の `view` に置いた `font-size` が深い `text` に漏れる（spooky）。RN 語彙の GPU 基盤として、また LLM が生成する `<view style={font-size:20}>` が無関係な text を restyle しない予測可能性のために、実物の Flutter の2チャネル設計に寄せる。

## Decision

テキスト継承を**2チャネル**にする。プロパティ**名**は CSS 準拠（Hayate CSS）を維持し、**適用セマンティクスを Flutter** に寄せる。

### チャネル1：通常 text スタイル（text-local / RN-strict）

`font-family` / `font-size` / `font-weight` / `color` / `font-style`（新）/ `text-decoration`（新）。

- text/span に適用。継承は **text→text（IFC 内の span chain）のみ**。`view` 等の block を**貫通しない**。
- `view` にこれらを置いても text に影響しない（view は text ではない）。
- 解決は `InlineText`（ADR-0063）の span cascade が担う。

### チャネル2：ambient Default Text Style（block 貫通）

`default-font-family` / `default-font-size` / `default-font-weight` / `default-color`（Flutter `DefaultTextStyle` 相当のフルセット＝本決定 (ii)）。

- 任意の element に設定でき、**block を貫通して top-down に降りる**。nested 上書き可。
- text 要素が明示値も text 継承値も持たないときの**既定値**を供給する（app 全体の既定フォント/サイズ/色）。
- `scene_build.walk` の既存 top-down 機構を**この用途に転用**（`InheritedStyle` の意味を「通常スタイルの継承」から「ambient 既定の供給」へ変更）。

### 解決順（text/span のプロパティ）

1. 自身の明示値
2. text 祖先からの継承（text→text・IFC 内）
3. ambient 既定（`default-*`・最寄り祖先・block 貫通）
4. ハード既定（Noto Sans / 16px / weight 400 / black）

## Consequences

- **継承ロジックが2チャネルに分離**：通常スタイル＝`InlineText`（1箇所）、ambient 既定＝`scene_build.walk`（1箇所）。CSS 式の「全要素で個別 font プロパティ継承」は廃止。
- `scene_build` の `InheritedStyle` を **ambient Default Text Style チャネル**に転用（運ぶ値の意味が変わる）。
- **新規 style prop**（protocol `style_tags` 正本＝spec proto に追加）：通常側 `font-style` / `text-decoration`、ambient 側 `default-font-family` / `default-font-size` / `default-font-weight` / `default-color`。`font-weight` も継承対象セット（チャネル1の text→text）に含める。
- `view` 等に置いた通常 text プロパティは text に漏れない（予測可能・LLM フレンドリー）。
- ADR-0063 の inline cascade は本 ADR の解決順 1–2 を担う（変更なし、明確化）。
- HTML Mode：通常スタイルは対象 text 要素にのみ CSS を当て、ambient 既定は祖先に当ててブラウザ継承に委ねる（要 mapping 検討、§8）。

## Considered Options

- **CSS 式（全要素で font プロパティ継承＝ADR-0047 現状）**：app 既定は楽だが layout コンテナの font が text に漏れる。2b・LLM 予測可能性で不利。supersede。
- **純 RN（text→text のみ・ambient 既定なし）**：予測可能だが app 全体の既定フォントを置けず冗長。
- **2チャネル（本決定）**：通常は text-local（RN）、ambient 既定のみ block 貫通（Flutter `DefaultTextStyle`）。両者の利点を取る。

## 関係

- ADR-0047: supersede（「全要素 text 継承」を「text-local ＋ ambient 既定」に置換）。
- ADR-0063（2b・IFC）: 解決順 1–2 を `InlineText` が担う。
- ADR-0049/0055（protocol 正本）: 新 style prop は spec proto に追加。
