# インタラクション擬似状態スタイルを Render Layer で解決する

**Status: accepted**

**Date: 2026-06-07**

**Supersedes: ADR-0019**（スタイル切替の責務分担に関する部分）

## Context

ADR-0019 はインタラクション状態（hover / active / focus）に応じた**スタイル切替**を上位層（Hayabusa / Tsubame Signal）の責務とし、Hayate は `hover-enter` / `hover-leave` 等のイベント通知のみを担う **event-driven 方式**を採用した。

しかし Canvas Mode での実装・利用を通じて、次の問題が構造的に露呈した。

1. **CSS/DOM エンジンとの非対称**: ブラウザは `:hover` / `:active` / `:focus` をレンダラーが解決する。Hayate HTML Mode も実質ブラウザに委譲している。Canvas Mode だけが「イベント → Signal → 次フレーム `setStyle`」という迂回経路を強いられ、ホバー領域内でのチラつき・1 フレーム以上の遅延・親子ネスト時の誤 `hover-leave` が発生する。
2. **ホバー判定セマンティクスの不一致**: `hit_test` は最深要素を返すが、CSS `:hover` は「自身または子孫のいずれかにポインタがある」祖先にもマッチする。この差が、親に `onHoverEnter` を付けたネスト UI で「まだカード内なのに親のホバーが外れる」原因になった。
3. **ADR-0019 の前提の過信**: 「最大 1 フレームのラグは人間に知覚不能」としていたが、`mousemove`（即時）と `flush → render → poll_events`（RAF 同期）の分離により、実際には複数フレームにまたがるスタイル遅延とイベント順序のずれが起きうる。

一方、ADR-0019 が render-layer 方式を却下した主因は次の 3 点だった。

| 却下理由（ADR-0019） | 本 ADR での見直し |
|----------------------|-------------------|
| ADR-0018 export poll モデルと衝突 | **衝突しない**。ポインタ状態の追跡は既に Platform Adapter / `RendererEventState` が担っており、render パス内で effective style を解決するだけでは import callback も host への能動 push も不要 |
| WIT が CSS エンジンに肥大化 | **段階導入で抑制**。Phase 1 は既存 `StyleProp` 語彙のコピーのみ（`:hover` / `:active` / `:focus`）。`transition` / `animation` は Phase 2 以降または Framework 責務のまま |
| Framework の演出自由度が下がる | **イベント通知は温存**。アプリロジック用の `hover-enter` / `hover-leave` は引き続き delivery 可能。スタイルは Render Layer がデフォルト解決し、Framework は上書き不要 |

DOM や CSS エンジンが担うのが普通であるという指摘は妥当である。Hayate の目標像は「Hayate tag + Hayate CSS を渡せば描画できる軽量 DOM」（ADR-0053）であり、擬似クラススタイルの解決はその DOM engine 責務に含めるのが自然である。

## Decision

**インタラクション擬似状態に対応するスタイル（`:hover` / `:active` / `:focus` 相当）を Hayate Element Layer の Render Layer で解決する。**

### 1. 責務分担（ADR-0019 からの変更点）

| 層 | 担うもの |
|----|----------|
| **Hayate Element Layer** | 擬似状態スタイルの保持・ポインタ状態に基づく effective style 解決・layout / scene_build への反映 |
| **Platform Adapter** | raw 入力 → document runtime への配送（現状維持） |
| **Framework（Tsubame / Hayabusa）** | 擬似状態スタイルの**宣言**（`element_set_pseudo_style` 等）。スタイルの**切替ロジック**は不要。アプリロジック用 listener は任意 |
| **DOM Renderer（Tsubame）** | ブラウザ native `:hover` 等にマップ、または Hayate pseudo style を CSS ルールに変換（実装詳細は Tsubame 側） |

### 2. CSS 互換のホバーセマンティクス

スタイル解決用の `:hover` 判定は **最深 hit 要素だけ**ではなく、次を満たす。

> 要素 E が `:hover` である ⇔ ポインタが E の境界内にある、または E の子孫のいずれかの境界内にある。

`hit_test`（click / wheel のターゲット決定）は引き続き最深要素を返す。`:hover` スタイル解決と hit target 決定は目的が異なるため分離する。

`active` は `active-start` を受けた要素（ドラッグ中は座標が外れても維持、ADR-0031 維持）。`focus` は `focused_element` に一致する要素。

### 3. 擬似状態の優先順位

effective visual は base style に対し、次の優先順位で上書きする（CSS と同順）。

```
base < :focus < :hover < :active
```

（`disabled` 等は将来拡張。Phase 1 では hover / active / focus の 3 つのみ。）

### 4. API（Phase 1）

WIT / `apply_mutations` に擬似状態スタイル設定を追加する。

```wit
// 概念スケッチ（具体名は proto/spec 追加時に確定）
enum pseudo-state { hover, active, focus }

element-set-pseudo-style: func(element-id: u64, pseudo: pseudo-state, props: list<style-prop>)
element-unset-pseudo-style: func(element-id: u64, pseudo: pseudo-state, kinds: list<style-prop-kind>)
```

- 設定可能プロパティは Phase 1 では base `StyleProp` と同一語彙（`background-color` / `border-color` / `opacity` 等）
- `transition` / `animation` は Phase 1 対象外

Tsubame Renderer Protocol 側は JSX での宣言を想定する。

```tsx
// 概念スケッチ
<view
  style={{ backgroundColor: COLORS.panel }}
  pseudoStyle={{
    hover: { backgroundColor: COLORS.panel2 },
    active: { backgroundColor: COLORS.panel3 },
  }}
/>
```

### 5. イベント通知（ADR-0019 からの温存）

`hover-enter` / `hover-leave` / `active-start` / `active-end` の delivery は**廃止しない**。

- **スタイル**: Render Layer が自動解決（Framework は Signal で切り替えない）
- **ロジック**: 「ホバー中だけツールチップを出す」等は引き続き listener で実装可能
- `hover-enter` / `hover-leave` の発火セマンティクスは `:hover` スタイル解決と同じ祖先チェーンに揃える（最深要素切替だけで親が leave しない）

### 6. ADR-0018 / ADR-0053 との関係

- **export poll 原則は維持**。Hayate は host を import しない
- **Element Document Runtime の責務を拡張**する（ADR-0053 の「Runtime が担わない :hover スタイル」を撤回）
- render タイミングでの内部状態解決であり、「Hayate がフレームループを握る」わけではない。host は引き続き `render()` を呼ぶ

## Considered Options

- **event-driven 方式を維持し実装だけ直す（却下）**: `poll_events` 順序の調整や hit_test の改善で一部は緩和できるが、Framework 全コードに Signal ベース hover が残り、DOM エンジンとの二重モデルが恒久化する
- **render-layer 全面採用 + イベント廃止（却下）**: アプリがホバー状態を参照するケース（ツールチップ・aria-live 等）で listener が必要
- **render-layer 採用 + イベント温存（採用）**: DOM/CSS と同型のスタイル解決を Render Layer に戻しつつ、ロジック用イベントは残す

## Consequences

- ADR-0019 の「スタイル切替は上位層の責務」は**撤回**。イベント通知の責務は温存
- `hayate-core` の `Element` に `pseudo_visual: { hover, active, focus }`（各 `Visual` パッチ）を追加し、layout / scene_build 前に effective `Visual` を合成する
- `RendererEventState::apply_hover` のセマンティクスを「最深要素の切替」から「`:hover` 集合の差分更新」に変更する
- Tsubame `hello-world` 等の `hovered` Signal + 毎フレーム `setStyle` パターンは**移行対象**（擬似スタイル宣言に置換）
- proto/spec に `pseudo_states` セクション（または opcodes 拡張）を追加し、generator で Rust / TS codec を生成する（ADR-0055 流儀）
- Phase 2 候補: `transition`、`:disabled`、pressed 状態のキーボード操作対応

## Implementation Phases

| Phase | 内容 |
|-------|------|
| **P0（本 ADR）** | 決定の文書化、ADR-0019 / ADR-0053 / CONTEXT の更新 |
| **P1** | `hayate-core` effective style 解決、Canvas Adapter 統合、基本テスト |
| **P2** | proto/spec + `apply_mutations` wire 拡張、Tsubame `pseudoStyle` API |
| **P3** | 既存デモ・hello-world の Signal hover パターンからの移行 |

## Related

- **Supersedes** ADR-0019（インタラクション状態スタイルの責務分担）
- **Amends** ADR-0053 — Runtime が `:hover` 等の擬似状態スタイル解決を担う
- **Maintains** ADR-0018 — export poll 原則
- **Maintains** ADR-0031 — `hover-enter` / `active-start` 等のセマンティックイベント名
