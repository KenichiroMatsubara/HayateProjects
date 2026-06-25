# §6 Event Model

イベント通知（export poll）、interaction 状態、擬似スタイル解決、Tsubame 側の購読規範。
delivery の wire 形式そのものは §10（PROTO-12〜17）に置く。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### EVT-01 — Export poll モデル
**規範文:** Hayate はイベント通知に export poll を採用する。import callback を持たず、host が `poll_events()` を呼んでキューを drain する（一方向依存）。
**出典:** ADR-0018, ADR-0053
**状況:** ✅ — `document_runtime.rs` の `poll_deliveries()`、adapter `poll_events()` export。
**備考:** ポーリング遅延は最大1フレーム（≤16ms）。

### EVT-02 — 擬似スタイルは Render Layer が解決
**規範文:** `:hover` / `:active` / `:focus` に応じた effective style は Hayate Render Layer が解決する。Framework が Signal でスタイルを切り替える設計は採らない。`:hover` は ancestor chain にも match する。
**出典:** ADR-0056（ADR-0019 を supersede）
**状況:** ✅ — `pseudo_state.rs` の `resolve_visual()`（focus→hover→active の順で base に上書き）、`hover_set_for_hit()`（ancestor chain）。interaction 追跡は `tree.rs` の `hovered_elements` / `active_element`。
**備考:** [履歴] ADR-0019「interaction state as event でフレームワークが切替」は撤回。

### EVT-03 — セマンティックイベント命名
**規範文:** ポインタイベントは物理名（pointer-enter/leave）ではなく状態遷移を示すセマンティック名（`hover-enter` / `hover-leave` / `active-start` / `active-end`）を使う。`pointer-move` のみ target なしの物理名を残す。
**出典:** ADR-0031
**状況:** ✅ — `event_kinds.json` / `event_types.rs` に hover_enter/leave・active_start/end、`PointerMove { x, y }`（target なし）。
**備考:** —

### EVT-04 — listener registry + bubble dispatch
**規範文:** `register_listener(element_id, kind) -> ListenerId` で登録し、runtime が `bubbles()` 可否に基づき dispatch する。bubbling（click / text-input / keydown）は target→root の ancestor chain、non-bubbling（focus / blur / composition_* / scroll / hover_* / active_* / resize）は target のみ。
**出典:** ADR-0053
**状況:** ✅ — `document_runtime.rs` の `register_listener` / `dispatch_to_path`、`event_types.rs` の `DocumentEventKind::bubbles()`。`ListenerId` は generational key。
**備考:** delivery の wire 化は §10 PROTO-12/13。

### EVT-05 — composition イベントは non-bubbling
**規範文:** IME composition（start/update/end）は non-bubbling で target（`text-input`）のみに dispatch する。Raw Layer ユーザーは IME を自前実装する。
**出典:** ADR-0017
**状況:** ✅ — `event_types.rs` で composition_* の `bubbles()=false`、`event_kinds.json` に `bubbles: false`。
**備考:** preedit 状態保持は §5 TEXT-07。

### EVT-06 — Tsubame Adapter は hover イベント購読を拒否
**規範文:** Tsubame Adapter は `onHoverEnter` / `onHoverLeave` JSX prop を開発時に throw で拒否する。視覚ホバーは Hayate CSS `:hover` ブロックのみ。Hayate の hover delivery は low-level host / 非 Tsubame client 向けに内部に残す。
**出典:** ADR-0059（ADR-0056 と整合）
**状況:** ✅ — イベント語彙（`EVENT_PROP` / `REJECTED_EVENT_PROPS = {onHoverEnter, onHoverLeave}`）は `Tsubame/packages/renderer-protocol/src/event.ts` に移管され全 adapter が共有（`event-vocabulary.test.ts`）。`tsubame-solid`（`events.ts`）・`tsubame-react`（`props.ts`/`events.ts`・`host-config.test.tsx`）が共通語彙で onHover* を throw 拒否、event mapping に onHover* なし。
**備考:** ホバー prop 拒否は renderer-protocol の共有語彙で solid/react に効く（`tsubame-vue` 実装時も同語彙で自動的に効く）。tsubame-vue は未実装（§11 TSUB-05）。

### EVT-07 — Interaction 状態機械は ElementTree が所有
**規範文:** 入力→セマンティックイベントの状態機械（focus/active/hover の所有、`on_pointer_down/up/move`・`on_key_down`・`on_wheel`・`on_text_input`・`on_composition_*`）は `ElementTree`（Element Document Runtime）が持つ。各メソッドは hit-test（必要時）＋状態遷移＋イベント生成/dispatch を core で行う。Platform Adapter は raw platform 入力を `tree.on_*` に翻訳し描画を flush するだけで、interaction 状態を複製しない。
**出典:** ADR-0066（ADR-0053 を完遂）
**状況:** ✅ — `interaction.rs` が `ElementTree::on_pointer_down/up/move`・`on_wheel`・`on_key_down`・`on_text_input`・`on_composition_*`・`on_hover_enter/leave`・`on_focus/blur` を提供。focus/active/hover は tree 単独所有。`hayate-adapter-web` は raw 入力→`tree.on_*` の薄い翻訳のみ（`RendererEventState` 撤去）。
**備考:** D2（`element_renderer.rs` god-module 分割）の前提を整える。Web/native adapter が同一 core 状態機械を共有。

### EVT-08 — pointer cursor は要素から解決し Platform Adapter が適用
**規範文:** `cursor` はポインタ直下の要素から解決し、`on_pointer_move` の出力（`PointerMoveResult.resolved_cursor`）として Platform Adapter に渡す。Adapter は OS/ブラウザのカーソルを駆動し、要素スタイルには触れない。coalesce された move（layout 未準備 / 1px dedup）では再計算せず直近値を持ち越す。Canvas adapter は生成 Hayate-CSS → browser-CSS マッパー（ADR-0070）を再利用し値リストを単一正本に保つ。
**出典:** ADR-0088、ADR-0066（interaction 状態機械）、ADR-0070（生成マッパー）
**状況:** ✅ — `style_tags.json` に `CURSOR`（enum = default/pointer/text/crosshair/not-allowed/grab/grabbing）、`CursorValue`。`interaction.rs` の `PointerMoveResult { moved, resolved_cursor }` ＋ `last_cursor` 持ち越し。Canvas adapter `apply_resolved_cursor`（`document.body.style.cursor`）。
**備考:** カーソルは要素ごと push でなくビューポート単位で1つ適用。DOM Mode は CSS `cursor` に直接写像。

### EVT-09 — transition は effective visual の変化を Render Layer が補間する
**規範文:** 要素の effective visual が変化したとき、Render Layer が変化前の表示値から target へ連続値プロパティ（`background-color` / `border-color` / `text-color` / `opacity` / `border-radius` / `border-width`）を `transition-duration`（ms）/ `transition-timing` に従って補間する。トリガは `resolve_effective`（ADR-0067）の per-property 差分で、擬似状態切替・`setStyle`・継承変化を区別しない（ブラウザ/Blink の computed-style 差分と同型）。enum・離散は target 即時。duration/timing は after-change（解決済み）の値を使う。`render(timestamp_ms)` の frame loop ＋ `visual_dirty`（ADR-0086/0032 を再利用）で進める。DOM はブラウザの CSS transition に委譲。
**出典:** ADR-0089（Render Layer 補間・frame loop・補間対象6連続値・DOM 委譲）、ADR-0093（トリガを resolve シームへ＝up-level パリティ・`from`=表示値の連続反転・per-property state・after-change duration）
**状況:** ✅ — トリガを `emit_element` の `resolve_effective` シームへ移設（ADR-0093、issue #227）。前フレームの表示値（post-blend）を retained 描画状態（`SceneLowering` の `AnchorEntry.last_displayed`）に memo し、解決値との per-property diff で `ElementTransitions`（要素×プロパティ単位）を起動/調整する。`from`=表示値で逆方向割り込みを連続反転、duration/timing は after-change 解決値、初回 emit・full ephemeral rebuild は補間しない。継承変化（祖先の `default-*`/text-style 変更が子孫の算出値を変える非局所変化）も同シームで拾い、子孫自身に解決される `transition-duration > 0` のとき子孫が補間する（issue #228）。`transition.rs` / `transition_interpolation.rs` / `transition_inheritance.rs`。
**備考:** up-level 化で Canvas/DOM の Semantics Parity（ADR-0002）破れ（setStyle 即時 vs 補間・逆方向ジャンプ vs 連続）が解消された。祖先と子孫が同時に補間する場合、毎フレームの再 mark は両者を `SelfOnly` で dirty にするため、`minimal_patch_roots` は reach を考慮して刈る — `SelfOnly`/`ZIndex` の祖先は自身しか再 emit しないので、その下の dirty な子孫は patch root として残す（さもないと子孫の補間が停止する、issue #228）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 9 | EVT-01〜09 |
| 🟡部分 | 0 | — |
| ⬜未実装 | 0 | — |
