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
**状況:** ✅ — `Tsubame/packages/solid/src/events.ts` の `REJECTED_EVENT_PROPS = {onHoverEnter, onHoverLeave}`。event mapping に onHover* なし。
**備考:** tsubame-vue/react は未実装のため当該検証は solid のみ（§11）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 6 | EVT-01〜06 |
