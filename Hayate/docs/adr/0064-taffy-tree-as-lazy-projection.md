# Taffy ツリーは ElementTree から lazy に派生する投影とする

**Status: accepted（ADR-0063 の前提として ElementTree↔Taffy 関係を定義。ADR-0004/0008 を継承）**

**Date: 2026-06-07**

## Context

現状、構造は Hayate の Rust メモリ内に**二重**にある：
- `element_create`（`tree.rs:188`）が**全 element に Taffy leaf を確保**。
- `element_append_child`（`tree.rs:602–603`）が `taffy.add_child(...)` と `el.children.push(...)` を**両方手で書く**。

document 構造（`ElementTree`）と layout ツリー（Taffy）が **peer として各 mutation site で手 sync** されている。

ADR-0063（2b・IFC）は Taffy を **1:1 ミラーから非自明な投影に変える**：inline span は Taffy ノードを持たない（`el.taffy_node: Option`）、IFC root は measured leaf、`text` を inline/block 境界をまたいで reparent するとクラスが反転する。**非自明な投影を全 mutation site で手書きするのは形が間違っている**（locality 税の散在）。

### 先行事例（RN / Flutter）

- **Flutter**: Widget / **Element**（構造 owner）/ **RenderObject**（layout・paint）の 3-tree。layout は RenderObject ツリーで行い、Element ツリーとは別。inline span（`TextSpan`）は RenderObject ではなく `RenderParagraph` 内データ。
- **React Native**: **Fiber**（構造 owner）/ **Yoga shadow tree**（layout）/ native views。Yoga は Taffy の同類で inline を持たず、nested `<Text>` は attributed string range に平坦化。RN は view flattening で構造ツリーと layout ツリーを 1:1 にしない。

両者とも **構造 owner と layout ツリーを分離**し、layout ツリーを構造から build/commit フェーズで reconcile する。「1本のツリー（Taffy を owner）」はどちらもやっていない外れ値であり、採れば document 意味論を layout エンジン API に結合し、テスト（document テストに `TaffyTree` が必要化）・layout 二層分離（ADR-0008）・inline span の表現（`Display::None` hack）を損なう。

## Decision

### owner は一人、Taffy は派生

- **`ElementTree` が唯一の owner** — document 構造（parent/children）と element データ（kind / visual / text / listeners / `layout_style`）を持つ。
- **Taffy は ElementTree の block-box 部分集合の derived projection** — inline span は除外、IFC root は measured leaf。peer ではなく、ElementTree から再構築/patch される。

### lazy 同期

- 構造 mutation（`element_create` / `append_child` / `insert_before` / `remove`）は **`ElementTree` だけを触り**、変更を **structure-dirty 集合**に記録する。**Taffy には触れない**（`tree.rs:602` 等の inline `taffy.add_child` を撤去）。
- Taffy 投影は **layout pass の冒頭（`LayoutPass::run`）で、dirty-scoped に reconcile** する（`compute_layout_with_measure` の前）。2b の投影規則（span 除外・IFC-leaf・reparent クラス反転）は**この1パスに集約**。
- mutation〜layout 間に Taffy 構造を読む経路は無い（読むのは layout 時と layout 後の `cache_layout`/hit-test）ため、stale でも無害。

### Taffy ハンドルの所在

- `el.taffy_node: Option<NodeId>`（block box のみ Some、span は None）。理想的には ElementId↔NodeId マップと Taffy ツリーを **`TaffyProjection` module が所有**し、`ElementTree` は Taffy ハンドルを構造/mutation レベルで持たない（`layout_style` は el が owner、投影時に Taffy ノードへ適用）。

## Consequences

- **`TaffyProjection`（hayate-core 内部 seam）** 新設：`TaffyTree` + ElementId↔NodeId マップを所有し、`reconcile(elements, dirty_set)` で block-box 部分集合を構築/patch、`compute_layout` を駆動。
- `element_create`/`append_child`/`insert_before`/`remove` は **純 ElementTree 操作 + structure-dirty 記録**に簡約。inline `taffy.*` 呼び出しを撤去。
- `LayoutPass::run`（`layout_pass.rs:51`）が `compute_layout` 前に `projection.reconcile()` を呼ぶ。
- `cache_layout` / `scene_build` / hit-test は geometry を投影経由（id→node→layout）で読む。inline span の geometry は IFC の `RangeMap`（Parley）由来。
- テスト面：document モデルのテストが Taffy 非依存になる（interface = test surface が素の `ElementTree`）。
- 却下: **Taffy-as-owner**（`TaffyTree<Element>`、span を `Display::None`）。1本化は美しいが document 意味論を Taffy API に結合し、テスト・層分離を損ない、span に意味流用 hack が要る。RN/Flutter も採らない。

## 関係

- ADR-0004（Taffy をエンジンとする）: 維持。Taffy は引き続き layout エンジン。
- ADR-0008 / §3 LAY-02（layout は Element Layer 内包）: 補強。owner（ElementTree）と layout 投影（Taffy）の分離を明文化。
- ADR-0063（2b・IFC）: 本 ADR が前提。2b が投影を非自明化したことが本決定の引き金。
