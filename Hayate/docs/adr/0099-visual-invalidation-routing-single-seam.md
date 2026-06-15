# visual invalidation の routing を単一 seam に集約する（#238 を延長）

**Status: accepted**

**Date: 2026-06-15**

## Context

`visual_invalidation.rs` の `classify` / `step_reach` は純関数で「何を / どこまで dirty にするか（WHAT・reach）」を決める（issue #238、ADR-0086 維持）。`ElementContext` を介して `ElementTree` を起動せず単体テスト可能で、reach テーブルがそのまま test surface になっている。

だが、`classify` が返す `Change`（dirty 種別＋reach）を**実際の dirty 集合へ撒く routing は `tree.rs` の 5+ 箇所に手書きで散在**する。`element_set_style` / `mark_pseudo_activation_dirty` / `mark_child_attachment_dirty` / `apply_visual` などが、それぞれ:

- `.dirty_kind` を読んで `engine.mark_visual_dirty` / `mark_shape_dirty` / `mark_structure_dirty` のどれかへ振り分け、
- shape のときは追加で `mark_text_content_dirty` を呼び、
- geometry に効くときは `layout.projection.mark_dirty(id)` も呼ぶ

——という対応を**個別に再記述**している。「`DirtyKind::Shape` は projection geometry mark も要求する」といった含意は型で強制されず、どれか一つを撒き忘れると visual と layout が静かに乖離する。reach の決定（`classify`）と routing が別ファイルにあるため、実バグは「正しく全 dirty 集合を撒けているか」という呼び出し側に潜む。

## Decision

**`classify` / `step_reach` の純粋性は保ったまま、`Change` → dirty 集合への routing を単一の関数に集約する。**

- `tree.rs` の各 `element_set_*` は従来どおり live tree の位相を読んで `ElementContext` を組み、「何が変わったか」を報告するだけ（#238 不変）。
- その報告（`Change`）を **1 つの routing 関数**へ渡すと、`ElementEngine`（visual / shape / structure）と `TaffyProjection`（geometry / projection）の該当する dirty 集合が **atomic に**マークされる。
- 「どの `Change` がどの dirty 集合へ届くか」の対応表（例: `Shape` は engine.shape ＋ projection geometry、`Structure` は engine.structure ＋ subtree 展開）は **routing 関数が単独で知る**。呼び出し側はもう個別の mark を並べない。

`classify`（WHAT）と `step_reach`（reach 伝播）が決め、routing が「決定を全 dirty 集合へ届ける」を 1 箇所で担う。

## Consequences

- **locality**：「`Change` がどの dirty 集合へ届くか」が 1 関数に集中。5+ の手書き routing site を廃止。
- visual / layout の乖離（撒き忘れ）が構造的に起きない。
- `classify` / `step_reach` の test surface（reach テーブル）は不変。routing は別途「`Change` → 撒かれた集合」のテーブルテストになり、純粋部とは独立に検証できる。
- `fonts_dirty` / `viewport_dirty` のような非 prop 駆動の無効化は従来どおり engine への直接 mark（routing の対象外）。

## Considered Options

- **routing を `classify` 裏に畳む**：`classify` が dirty marking まで担う。#238 の「`tree` は報告のみ・`classify` は純粋」を上書きし、reach テーブルの純粋 test surface を失う。却下。
- **現状維持**：routing が 5+ 箇所に散在。撒き忘れによる visual/layout 乖離が型で防げないまま。却下。

## 関係

- issue #238（reach を純関数 `classify` / `step_reach` に集約）を延長。
- ADR-0086（retained incremental lowering）維持。
- ADR-0075（`ElementEngine` が dirty 集合を集約・保管）と整合。
