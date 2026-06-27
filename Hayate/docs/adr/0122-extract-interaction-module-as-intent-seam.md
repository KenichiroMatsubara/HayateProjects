# `ElementTree` から `Interaction` を intent seam として切り出す：flat `InteractionIntent` 封筒・幾何は seam の裏・accessibility inbound は consumer

status: accepted

Date: 2026-06-28

## Context

`ElementTree`（`tree.rs` 2304 行）は ~20 フィールドの god-object で、`impl ElementTree` が
`tree.rs` / `interaction.rs`（1920 行）/ `accessibility.rs`（999 行）に分散している。interaction
状態機械（focus / hover / active / selection / `PointerGesture` / pointer-pos / modality / touch scroll）は
`ElementTree` のフィールドに直接ぶら下がり、`interaction.rs` は **テスト 0**。純粋な部品（`edit_state.rs`・
`pointer_gesture.rs`）は単体テスト済みだが、それらを**呼び出すコード**（縦移動・selection drag・edit drag）は
full tree ＋ `commit_frame` ＋ `render` を立ち上げないと到達できず、バグはこの「呼び出され方」に潜む。

決めること:

1. 切り出した module の入口（interface）の形。
2. accessibility（`accessibility.rs`）を同時に切り出すか、どう関係づけるか。
3. interaction state / `Selection` / per-element `EditState` の所有境界。
4. 幾何依存の edit 操作（縦移動・Home/End・point→byte）を full tree なしでテスト可能にする方法。

前提となる既存決定:

- ADR-0066：interaction 状態機械（focus/active/hover の所有と入力 surface）は Element Document Runtime が持つ。
- ADR-0103：text-input 編集は閉じた `EditIntent` 語彙で、Element Document Runtime が `EditState` に適用する唯一の編集シーム。OS キーバインドは Platform Adapter が `EditIntent` へ翻訳する。
- AccessKit / Accessibility Action（CONTEXT.md）：AT の inbound アクションは Core が**既存の runtime intent**（Click・focus 遷移・統一 Selection・Scroll Offset）へ写像し、**合成 pointer/key の replay は経由しない**（Flutter の semantic action と同型）。
- ADR-0003：単一スレッド Core。

## Decision

### 1. `Interaction` を deep module として切り出し、`apply_intent` 単一 seam を公開する

- `Interaction` は横断的 interaction state（focus / hover / active / press 位置 / `PointerGesture` /
  `PointerKind` / `InputModality` / 直近 pointer 位置・cursor / touch scroll）を所有する。
- element 位相・layout 幾何・per-element `EditState`・scroll offset は所有せず、狭い `InteractionTreeView`
  trait（`kind` / `layout_rect` / `ancestor_scroll_view` / `edit_state_mut` / `scroll_offset` / `emit_event` /
  `mark_dirty` 等）越しに借りる。Event と dirty mark は sink 経由で出す。
- **interface = test surface**：fake な tree view を与えて intent を流し込み、状態と発火 Event を検証する。
  ADR-0066 が runtime 所有とした interaction 状態の所在を、この module へ精緻化する（runtime 内の再配置）。

### 2. 入口は flat `InteractionIntent` 封筒。`EditIntent` は内包し再定義しない

- `apply_intent(InteractionIntent)` 単一メソッドを seam とする。`InteractionIntent` は flat dispatch 封筒で、
  `Edit(EditIntent)` arm が既存の `EditIntent`（ADR-0103）を**そのまま内包**し、`Focus` / `Click` / `SetValue` /
  `ScrollToReveal` 等を並立 arm として持つ。`EditIntent` の「edit 専用シーム」という意味は不変。
- intent を**データ**にすることで、pointer/key 経路と AccessKit 経路が同一の値型を生産する。intent 値は
  どちらの adapter も起動せず構築・テストでき、**2 producer = 本物の seam**。
- 名前付きメソッド N 個（`transition_focus` / `semantic_click` / …）を公開する案は却下（テスト面が広がり、
  producer 共通性が型に出ず、adapter がメソッド集合に結合する）。

### 3. accessibility は `Interaction` の兄弟ではない。inbound = seam の consumer、outbound = 別 projection

- **inbound**（`on_accessibility_action`）：state を所有せず、既に `transition_focus`（`interaction.rs`）等へ委譲
  している。`map_action_request`（既に純関数）が `ActionRequest` → `InteractionIntent` を作り、`apply_intent`
  を通すだけの薄い **adapter** に潰す。これが「2 adapter = 本物の seam」の最強形（pointer/key adapter ＋ AccessKit
  adapter が同一 seam に intent を流す）。Accessibility Action が合成 pointer/key を replay しない原則と一致。
- **outbound**（`accessibility_update`）：`focused_element` ＋ tree ＋ layout を読んで AccessKit `TreeUpdate` を吐く
  純粋 reader ＝ **projection**。scene lowering / `Taffy Projection` の仲間で、`Interaction` の peer ではない。
  state を持たず deletion test が弱いため、切り出しは**低優先の後続**（別 issue）。
- accessibility を「state に触る第二の兄弟」として切り出すのは却下。両 module が focus/selection/scroll に
  独立に手を伸ばすと、god-object 内の漏れを 2 ファイルに分散させるだけ（intent 適用ロジックが複製として再出現）。

### 4. `Selection` は deep module 抽出、`EditState` は既に deep module（ラッパー新設せず）

- **`Selection`**：`selection.rs` に正規化・縮退・contains 境界 clamp の不変条件を集め、drag-select / extend /
  collapse / clear を interface の裏へ。物理 storage は `Interaction` の field に置き、read 経路（scene_build の
  Selection Chrome 等）は interface 越しに borrow する。不変条件が interface の裏に入ることで「どの struct が
  field を持つか」は borrow checker の都合に降格する。「Element Document Runtime が単独所有」は runtime 内の
  再配置であり不変。
- **`EditState`**：`edit_state.rs`（1109 行）は `EditIntent` を純粋適用し単体テスト済みで、型として既に deep。
  `Map<ElementId, EditState>` 上の `EditStore` ラッパーは新設しない（deletion test に落ちる薄いラッパー）。
  storage は `Element.edit` のまま、`InteractionTreeView::edit_state_mut(id)` で借りる。
- **共有所有（`Rc<RefCell<_>>`）は却下**：単一書き手のコンパイル時保証を捨て aliasing と実行時 borrow panic を
  招き、共有セルのセットアップでテスト面をむしろ悪化させる。depth で所有問題を溶かす（共有 mutability ではなく）。

### 5. 幾何依存 edit 操作は `Caret Geometry` ビューを注入して純粋計算する

- 縦移動 ↑↓・表示行 Home/End・point→byte の hit は、解決済みの行レイアウトビュー `Caret Geometry`
  （`line_of` / `x_of` / `byte_at_x_on_line` / `line_bounds` / `byte_at_point`）を `EditState` 操作に**注入**して
  計算する。実 adapter は Parley（`content_layout`）を包む 1 つ、test adapter は行→byte 範囲・byte→x の
  手書きテーブル 1 つ（ここでも 2 adapter = 本物の seam）。これにより縦移動等が full tree なしで純粋にテスト可能。
- goal column（`edit.desired_x`）は `EditState` 側に残す（横移動で更新・縦移動で消費）。
- 移動解決を `Interaction` 側に残し `EditState` を byte-mover に留める案は却下：テストしたい挙動（縦移動）が
  「テストできない部分」として `Interaction` に居残るため、本 ADR の動機に反する。

## Considered Options

- **入口を名前付きメソッド N 個にする**：seam が impl のメソッド面になり、adapter がメソッド集合に結合。
  producer 共通性が型に出ない。flat `InteractionIntent` 封筒を採用。
- **`EditIntent` を広げて focus/click/scroll も飲ませる**：ADR-0103 の「edit 専用シーム」の再定義になる。
  上位封筒で内包するに留める。
- **accessibility を state 所有の第二 module として切り出す**：漏れを 2 ファイルに分散。inbound=consumer に倒す。
- **`Selection` / `EditState` を `Rc<RefCell<_>>` で共有所有**：aliasing・実行時 panic・テスト面悪化。depth で解消。
- **`EditStore` ラッパー新設**：HashMap 上の薄いラッパーで deletion test に落ちる。trait 借りに留める。
- **幾何移動解決を `Interaction` に残す**：テストしたい挙動が untested 領域に残る。`Caret Geometry` 注入を採用。

## Consequences

- `interaction.rs` の interaction 状態機械が `Interaction` module（`apply_intent` 1 seam）になり、fake tree view
  ＋ intent 値で単体テスト可能になる。今テスト 0 の最大領域に test surface が付く。
- accessibility inbound は `map_action_request` → `InteractionIntent` の薄い adapter に縮み、AccessKit 経路と
  pointer/key 経路が同一 seam を共有する。outbound projection の切り出しは別 issue（低優先）。
- `Selection` は deep module 化、`EditState` は配線変更のみ（型は不変）。`Caret Geometry` seam が新設され、
  Parley 依存が edit 移動ロジックから切り離される。
- 後続候補（invalidation seam の完成：`shape_target` / `apply_change_at` を `visual_invalidation` へ）は、
  interaction を抜いた後のより小さな move として実施しやすくなる。
- `CONTEXT.md` に `Interaction` / `InteractionIntent` / `Caret Geometry` を追加、`Selection` 定義へ再配置注記。

## 関係

- ADR-0066（interaction 状態機械は Element Document Runtime が所有）：本 ADR はその状態の所在を runtime 内の
  `Interaction` module へ精緻化する（外部化ではない）。
- ADR-0103（text-input 編集 = `EditIntent` 語彙）：`EditIntent` を `InteractionIntent::Edit` arm として内包し、
  edit 専用シームの意味を保つ。
- ADR-0086（retained incremental lowering）／`visual_invalidation` の reach seam：accessibility outbound と
  scene lowering を projection の仲間として位置づける背景。後続の invalidation seam 完成の前提。
- ADR-0003（単一スレッド Core）：共有所有（`Rc<RefCell>`）を避け、depth ＋ 単一書き手で所有問題を解く根拠。
