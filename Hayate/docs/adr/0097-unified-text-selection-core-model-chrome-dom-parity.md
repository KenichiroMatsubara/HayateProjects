---
status: accepted
---

# 統一テキスト選択：core 所有の Selection モデル・core 描画のテーマ切替 chrome・DOM はネイティブ選択で意味論のみパリティ

**Date: 2026-06-14**

> **更新（2026-06-18）**: 決定2（`selectable` opt-in 領域）は **ADR-0108 が supersede**
> した（CSS `user-select` パリティ：element-kind UA 既定で既定選択可・`user-select` property・
> cross-element 選択・`contains` 境界）。決定5は ADR-0108 で精緻化。決定1/3/4 は有効。

## Context

Canvas Mode はキャンバスに GPU 描画するため、ブラウザのネイティブ文字選択が一切効かない（Flutter が `SelectionArea` を自前で持つのと同じ事情）。一方で text-input の編集選択（`EditState`・ADR-0069）と、読み取り専用の表示テキスト選択（Flutter の `SelectionArea` 相当）は別物として求められる。両者を「Flutter のような文字選択」として一貫した UX で提供したい。

Hayate には Canonical Tree（描画・layout・hit-test の単一正本）と Semantics Parity（Hayate CSS が正準、DOM 系がブラウザ既定を抑制・補完して合わせる）の原則があり、選択の所有・描画・パリティ境界をどこに引くかが論点になる。

## Decision

1. **統一 Selection モデルを Element Document Runtime（core）が所有する。** 読み取り選択と編集選択を単一の `Selection`（anchor / focus を `(ElementId, byte offset)` で表し document 順に正規化した連続範囲）で表現する。アプリ全体で同時に有効な選択は一つだけ。単一キャレット（`EditState.cursor_byte_index`）は anchor=focus の縮退形。interaction 状態機械（ADR-0066）と同型の runtime 内部状態とする。

2. **選択の境界は `view` の `selectable` typed property で確立する（Selection Region）。** ADR-0096 の `draggable` / `dropTarget` と同型の closed typed property（ADR-0071）。`selectable` な subtree 内でのみ連続選択が成立し、複数 block をまたいで広がる。nested は最寄り祖先が有効。text-input は境界に依らず常に選択可能。専用 element-kind は追加しない。

3. **選択 chrome（highlight・handle・floating toolbar）は core が SceneGraph に一度だけ描画し、スタイルのみテーマ切替する。** OS ネイティブ widget を Platform Adapter ごとに再実装しない。Material 流を先行実装し、Cupertino 流は iOS Platform Adapter を作る際に同時追加する（切替 enum で追加は additive）。拡大鏡（magnifier）は将来。Platform Adapter は clipboard 読み書きと raw 入力のみを担う。

4. **入力は既存 runtime intake（`on_pointer_*`・ADR-0066/0088）の内部挙動として実装する。** active-session の暗黙 capture を選択ドラッグへ拡張する。ADR-0096 の公開 pointer / DnD 語彙の実装は前提にしない。

5. **DOM 経路（HTML Mode / DOM Renderer）はブラウザネイティブ選択を使い、`user-select` で `selectable` 領域に拘束する。** パリティ契約は「何が選択可能か・Selection Region の境界」の意味論のみとし、handle / toolbar / magnifier の chrome 見た目はネイティブに委ねる。core モデルを DOM 側で再実装しない。

## Considered Options

- **DOM でも core モデルを再実装（厳密パリティ）** — `user-select: none` で抑制し core の Selection・highlight・chrome を DOM でも再現。意味論パリティは完全だが、ブラウザの優れたネイティブ選択を捨てて高コストに再実装することになる。chrome までパリティ対象にする実益が薄く却下。
- **選択 chrome を Platform Adapter ごとにネイティブ描画** — iOS / Android で完全ネイティブな見た目になるが実装が N 倍・OS 間で挙動が割れる。Flutter も chrome は自前描画＋テーマ切替であり、これに倣って却下。
- **全 text を既定で選択可能（境界なし）** — ボタン label 等の UI チクロムまで選択可能になる。Flutter が `SelectionArea` で明示境界を要求するのに倣い却下。
- **ADR-0096 の公開 pointer 層を先に実装してその上に選択を構築** — スコープと依存が広がる。選択は hover/active と同型の runtime 挙動として既存 intake に乗せられるため不要。

## Consequences

- `proto/spec` に `selectable` element property を追加する（ADR-0071 closed vocabulary）。
- core に Selection モデル・hit-test（point→`(ElementId, byte)`）・highlight lowering・chrome 描画・clipboard 連携（Platform Adapter 経由）が新設される。
- DOM Renderer / HTML Mode は `user-select` 写像と `selectable` 領域の制御を追加するが、選択ロジック本体は持たない。
- `EditState`（ADR-0069）の `cursor_byte_index` は Selection の縮退形として統一モデルに吸収される成長点になる。
- `selection-change` イベントや programmatic 選択 API は MVP 範囲外（必要時に `event_kinds` へ追加）。
- 拡大鏡・Cupertino chrome は後続作業。chrome スタイルは初日から切替 enum とし、追加が書き直しにならないようにする。
