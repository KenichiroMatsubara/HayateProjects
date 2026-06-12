# Effective Style 解決を単一 seam に集約し query 露出する（候補 C1）

**Status: superseded（effective resolver seam は維持。scene lowering の「毎フレーム捨て」前提は ADR-0086 で置換）**

**Date: 2026-06-07**

## Context

`resolve_visual(base, pseudo, interaction, id) -> Visual`（`pseudo_state.rs:128`）は pure だが、`scene_build.rs:64` の walk 内でのみ呼ばれ、結果を SceneGraph に emit して**毎フレーム捨てる**。`ElementTree` から「この element の実効スタイルは？」を聞く public メソッドが無く、**test surface が SceneGraph**（`:hover` を試すには `render()` → SceneGraph walk）。

さらに今セッションの決定で実効スタイル解決が **3 箇所に分散**した：

- `scene_build.walk`：ambient 既定（ADR-0065 channel 2）＋ `resolve_visual`（pseudo）
- `InlineText`：text-local cascade（ADR-0063/0065 channel 1）
- `pseudo_state::resolve_visual`：pseudo merge

「実効スタイルがどう決まるか」を追うのに 3 ファイルを往復する（locality 欠如）。pure 関数は切り出されているが、**実バグは「正しい base/inherited/pseudo を組み立てて呼べているか」に潜む**。

## Decision

**per-element 実効スタイル解決を1つの shared seam に集約する。**

```
resolve_effective(inherited_ctx, own_visual, pseudo, interaction, id) -> Visual
  // パイプライン: 継承(ch1 text-local + ch2 ambient) → 自身明示 → pseudo(focus<hover<active)
```

- **query 露出**：`ElementTree::element_effective_visual(id) -> Visual`。inherited_ctx を ancestor walk で組み立て、shared resolver を呼ぶ。
- **3 caller が同一 resolver を共有**：`scene_build`（box/visual、継承は top-down threading で O(n)）、`InlineText`（inline text element の text-style、IFC 内 batch）、`element_effective_visual`（単体 query/test）。**継承 context の取得方法は caller で違う**（threaded / IFC threaded / ancestor-walk）が、**「context + 自身 + pseudo → effective」の中核は1関数**。一つの解決セマンティクス、複数の context 取得経路。
- **effective visual に限定**（visual/text フィールドのみ。layout prop は skip ＝ Taffy 領分、`apply_visual_props` の `is_layout` guard と整合）。

## Consequences

- **test surface が SceneGraph → Visual**。`:hover` test が1行：style 設定 → hover 設定 → `element_effective_visual` → assert。
- **locality**：実効スタイルのセマンティクスが1 resolver に集中。3 分散が解消。
- **leverage**：hit-test 精緻化・AccessKit・debug overlay が同じ query を共有。
- `InlineText` の inline text element ごとの text-style 解決 ＝ この shared resolver（表示 shaping と query が同一解決）。
- ADR-0056（pseudo）・ADR-0065（2チャネル継承）・ADR-0063（InlineText）の解決を統合。

## Considered Options

- **minimal（pseudo だけの query）**：`element_effective_visual` を base＋pseudo に限定し継承は scene_build/InlineText に残す。query は増えるが継承解決が分散のまま＝ C1 の病巣が残る。却下。
- **現状維持**：実効スタイルが scene_build に transient。query/test 不可・3 分散。

## 関係

- ADR-0056：擬似スタイル解決（pseudo）を本 resolver が内包。
- ADR-0065：2チャネル継承の解決順 1–3 を本 resolver が実行。
- ADR-0063：`InlineText` が本 resolver を per-inline-text-element に適用。
