# Element→scene lowering を dirty-gated incremental にし Element Anchor を導入する（ADR-0067 を supersede）

**Status: accepted（HITL — foundation slice L1, issue #182）**

**Date: 2026-06-12**

## Context

`ElementTree::render()` は毎フレーム `scene_build` で Canonical Tree を全走査し、新しい `NodeId` を毎回払い出していた。`scene_cache` フィールドはあるが実質 Immediate（allocate-and-discard）であり、語彙の **Retained** は志向のみで実装されていなかった。既存の dirty 集合（`structure_dirty` / `shape_dirty` / `viewport_dirty`）は Taffy projection と Parley shaping を駆動するが、scene lowering には未接続だった。

ADR-0067 は effective style を単一 resolver に集約したが、「`scene_build` walk 内で解決して毎フレーム捨てる」前提を残していた。本スライスは **how（scene の生成方法）** を Retained incremental に切り替え、**what（effective visual のセマンティクス）** は ADR-0067 の resolver をそのまま使う。

## Decision

1. **Element Anchor（要素アンカー）** — `NodeKind::ElementAnchor { element_id }` を導入する。transform を持たない構造専用 Node で、element ごとの retained identity を担う。親 anchor の子リストは子 element の anchor を参照し、子の内部変更は親 anchor を触らない（locality）。

2. **Retained incremental lowering** — `SceneLowering`（`ElementId → anchor` マップ + `built` フラグ）を `ElementTree` に保持する。`render()` は dirty snapshot（structure / shape / viewport / `visual_dirty` / interaction 変化 / cursor blink）を取り、dirty が空なら **re-lowering walk ゼロ**で `scene_cache` を再利用する。dirty があるときは minimal patch roots から部分 re-lower する。

3. **`visual_dirty`** — scene-only の視覚変更用集合を `ElementEngine` に追加する。`element_set_style` の非 layout・非 text 変更、transform / scroll / image / focus / blur 等がマークする。viewport variant の非 text 解決も `promote_viewport_dirty` 経由でマークする。**closed-world completeness**（全 visual mutator が dirty をマークする不変条件）は次スライスで強制する（本 ADR は仕組みのみ）。

4. **Paint parity** — `render_scene_graph` は Element Anchor を透過 walk する。draw op 出力は ephemeral full rebuild（`build_ephemeral`）と一致することをテストで保証する。

5. **Pseudo-state on dirty pipeline** — hover / active / focus 変化は interaction snapshot 差分で pseudo 付き element を scene dirty に含める。擬似状態の dirty パイプライン統合本体は後続スライス。

## Consequences

- SceneGraph は Element Anchor を含むが、描画結果（`DrawOp`）は従来と同等。
- クリーンフレームでは lowering walk count がゼロ（measurable）。
- 子のみ visual dirty のとき親 anchor の `NodeId` は不変。
- ADR-0067 の「毎フレーム捨てる lowering」記述は本 ADR で置換。`element_effective_visual` query seam は維持。

## Considered Options

- **毎フレーム full rebuild のまま anchor だけ追加** — retained identity は得られるが dirty 未接続で CPU コストは不変。却下。
- **`visual_dirty` を次スライスまで延期** — background-only 変更や viewport variant が stale になり parity テストが壊れる。foundation で仕組みを入れる。

## 関係

- **supersedes** ADR-0067 の scene lowering 前提（effective resolver 自体は維持）。
- ADR-0075：`ElementEngine` dirty 集合の延長（`visual_dirty`）。
- ADR-0054 / ADR-0079：paint walk と golden parity の安全網は不変。
