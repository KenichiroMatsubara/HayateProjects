# interaction 状態遷移と pseudo-state invalidation を同一操作にする（ADR-0066 を延長）

**Status: accepted**

**Date: 2026-06-15**

## Context

ADR-0066 は interaction 状態機械（focus / active / hover の単独所有と `on_pointer_*` / `on_key_down` / `on_wheel` / `on_text_input` / `on_composition_*` の入力 surface）を `ElementTree` に置くと決めた。状態は `ElementTree` の 9 つの field（`focused_element` / `hovered_elements` / `active_element` / `selection` / `selection_drag` / `edit_drag` / `last_click_pos` / `click_count` / `last_pointer_pos` ＋ `last_cursor`）に保持される。

これらの状態を遷移させる `interaction.rs` のメソッドは、状態を flip した後、対応する pseudo-state の無効化を**手書きで別行として**撒いている。例（`pointer_down_on_target`）:

```rust
self.mark_pseudo_activation_dirty(t, PseudoState::Active);
self.active_element = Some(t);
```

flip（`active_element = ...`）と invalidation（`mark_pseudo_activation_dirty`）が分離した 2 文で、**両方が必ず起きる保証が型に無い**。片方を撒き忘れると `:active` / `:hover` / `:focus` のスタイルと interaction 状態が静かに乖離する。`hover_enter` / `hover_leave` / `blur` など複数のジェスチャ経路で同じ対が繰り返され、撒き忘れの面が広い。

### module 分割は採らない（deletion test）

「interaction 状態機械を `ElementTree` から別 module に分離する」案は **deletion test を通らない**。interaction メソッドは tree から不可分で、`hit_test` は `layout` ＋ `elements`、selection 編集は element の text / IFC、状態 flip は `engine` ＋ `projection` への dirty mark を要する。状態は tree 上の view であって独立した物ではないため、9 field を構造体に切り出してもメソッドは `(&mut interaction_state, &tree)` を両取りし、**複雑さは集約されず移動するだけ**。`Interaction Engine` という名詞は語彙（CONTEXT）に無く、導入しない。

## Decision

**interaction 状態の遷移と、それに対応する pseudo-state invalidation を、分離不能な単一操作にする。**

- 各状態 flip（active / focus / hover の set/clear）は、対応する `mark_pseudo_activation_dirty` を**同じ操作の内側**で行う。呼び出し側が 2 文を手書きで対にすることはなくなる。
- 不変条件：**interaction 状態が変われば、その pseudo-state invalidation は必ず同時に撒かれる**。両者を別々に呼べる経路を残さない。
- module の抽出・新しい名詞の導入はしない。ADR-0066 の「状態機械は `ElementTree` が所有」を維持したまま、状態遷移の粒度だけを atomic にする延長。

## Consequences

- **locality**：`:active` / `:hover` / `:focus` のスタイルと interaction 状態の乖離が構造的に起きない。撒き忘れの面が消える。
- 複数のジェスチャ経路（pointer down/up・hover enter/leave・focus/blur）が同じ atomic 操作を共有する（leverage）。
- 状態と invalidation が 1 操作なので、ジェスチャのテストが「flip した → 該当 pseudo が dirty になった」を 1 呼び出しで検証できる。
- ADR-0099（invalidation routing の単一 seam）と整合：atomic 操作の内側で撒く先は ADR-0099 の routing 関数を通る。

## Considered Options

- **module 分割（interaction 状態を別 struct/engine に抽出）**：deletion test を通らない（上記 Context 参照）。複雑さが移動するだけで集約されない。却下。
- **現状維持**：flip と invalidation が手書きの 2 文。撒き忘れによる状態/スタイル乖離が型で防げない。却下。

## 関係

- ADR-0066（interaction 状態機械を `ElementTree` に置く）を延長。所有は変えず、遷移の粒度を atomic 化。
- ADR-0099（visual invalidation の routing を単一 seam に集約）と整合。
- ADR-0056（擬似スタイル解決）が、ここで atomic に撒かれた dirty を render 時に解決する。
