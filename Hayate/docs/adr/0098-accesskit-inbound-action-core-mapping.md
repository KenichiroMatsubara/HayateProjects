---
status: accepted
---

# AccessKit inbound action：Core 所有の意味論写像・ポインタ非合成・専用 NodeId 空間・統一 Selection 着地

**Date: 2026-06-15**

## Context

AccessKit は双方向プロトコルである。これまで Hayate は outbound（Core が `accesskit::TreeUpdate` を生成し Platform Adapter が AT に報告、PLAT-03 / ADR-0041）だけを実装しており、AT → Core の inbound アクション（スクリーンリーダのダブルタップ＝起動、フォーカス移動、値入力、選択移動、scroll into view）を受ける経路が語彙にも ADR にも存在しなかった。

inbound を設計するうえで、(1) アクションをどう runtime へ流すか、(2) 写像をどこが所有するか、(3) アクション語彙を Tsubame の proto contract に載せるか、(4) `SetTextSelection` を既存の統一 Selection（ADR-0097）とどう噛ませるか、(5) AccessKit `NodeId` 空間と Hayate の id をどう対応させるか、が論点になる。Flutter の semantics（semantic action は合成ポインタではなく、ノードに登録された intent ハンドラを `performAction` で直接叩く二重チャネル）を参照モデルとした。

## Decision

1. **inbound アクションは合成入力の replay ではなく、既存 runtime intent を直接駆動する。** `Action::Click`/`Default` は既存の `Click` イベント（bubble・listener dispatch 済み）を対象ノードへ直接 emit する。ヒットテスト・`:active` フラッシュ・multi-click カウンタ・focus 遷移ジェスチャを経由しない。座標を持たない semantic activation には node の layout 中心を入れる（wire 変更なし・既存リスナ互換）。Flutter の semantic action と同型。

2. **アクション → runtime 操作の写像は Core が単独所有する。** Core に inbound surface `on_accessibility_action(req)` を設け、Platform Adapter（UIA / NSAccessibility / AT-SPI）は OS の AT 配管として `(node, action, data)` を Core へ橋渡しするだけに痩せる。outbound（role/bounds 生成）が既に Core にある以上、inbound だけ adapter に置くと a11y セマンティクスが2箇所に裂け、adapter ごとの方言を生む。「Click がボタンに対して何を意味するか」はプラットフォーム非依存＝意味論パリティの対象。

3. **inbound アクション語彙は proto contract に載せず、Core 内 Rust API に留める。** AccessKit inbound はネイティブ adapter 専用で Tsubame の wire を渡らない（DOM 系は a11y をブラウザに委譲、Canvas a11y は ADR-0041 で凍結）。Core が `accesskit::Action` をサポート部分集合を表す Core enum `AccessibilityAction` へ写し、未対応 variant（`Increment` / `ShowContextMenu` / `Custom` 等）は入口で `Ignored` に畳む（total・test 容易）。

4. **最初のネイティブマイルストーンのアクション集合は {Focus, Click/Default, ScrollIntoView, SetValue}。** この4つで「読み上げ→ナビ→起動→値入力」の screen reader 基本ループが閉じる。`SetTextSelection` は text run 単位の a11y ノード（Parley `LayoutAccessibility`・既に future 凍結）に依存するため、その導入と同一作業単位に束ねて defer する。

5. **AccessKit `NodeId` 空間は Hayate 所有の専用 dense `AccessIndex` で構成し、host の `ElementId` から切り離す。** 要素ごとに単調な `AccessIndex` を払い出し、対応は a11y サブシステム側の bimap（`ElementId ⇄ AccessIndex`、逆引きは `Vec<ElementId>`）が持つ。`Element` 構造体に a11y フィールドは生やさない。Parley の TextRun ノードは `next_node_id` クロージャ経由で `(AccessIndex << k) | local`（local=0 を要素自身に予約、TextRun は 1..）にパックし、inbound は `node_id >> k` で所有 `AccessIndex` を O(1) 復元 → `Vec<ElementId>` → 要素の Parley マップへ委譲する。run の逆引きグローバルマップを持たない。

6. **`SetTextSelection` は統一 Selection（ADR-0097）に直接着地する。** `Cursor::from_access_position(layout, access, pos)` で AccessKit `TextPosition`（run NodeId + character_index）→ byte を解決し、`Selection { SelectionPoint(ElementId, byte), ... }` 一本を構成する。text-input でも read region でも単一の `Selection` に着地し、アクション層は要素種で分岐しない。text-input のキャレット/範囲は「両端がその text-input 内にある Selection」の縮退形であり、`EditState.cursor_byte_index` はその射影になる（ADR-0097 の成長点）。検証ルール：Selection Region 境界を跨ぐ範囲は clamp/reject、text-input の内外を跨ぐ範囲は不正。

7. **outbound の選択反映を inbound と対称に実装する。** 選択変化（pointer / key / AT アクション、origin 不問）はテキストコンテナノードの `node.set_text_selection(...)` で AT に読み戻す。位置は `Cursor::to_access_position` で `(ElementId, byte)` → `(run NodeId, character_index)` に戻し、Decision 5 と同じ NodeId パッキングを共有する。これは Decision 4 の text-run a11y と同一作業単位で着地する。

## Considered Options

- **inbound `Click` を合成ポインタで replay** — node 中心で `on_pointer_down/up` を叩き「実物のタップと同一」にする。だが座標逆算・`:active` フラッシュ・multi-click カウンタの誤起動という固有のバグ源を抱え、Flutter が semantics tap を合成入力にしない設計に反する。却下。
- **写像を Platform Adapter が所有** — プラットフォームごとにアクション解釈を変えられるが、その差は AccessKit 自身が吸収する層であり二重になる。outbound と inbound が裂ける。却下。
- **アクション語彙を proto `event_kinds` に追加** — 将来 Canvas a11y で JS から同語彙を送る布石になるが、native-only の概念を Hayate–Tsubame contract に混ぜるのは早すぎる一般化。Canvas a11y が来たら別の wire 拡張として設計する。却下。
- **NodeId を host `ElementId` に直接ビットパック（`ElementId << k | run`）** — side table 不要で純関数的に見えたが、`ElementId::from_u64` が host 採番の u64 全域を許すため「全 SDK が連番採番」という暗黙契約に健全性が依存し、他言語 SDK / Rust Element Layer SDK で静かに破綻する。さらに Parley が安定 NodeId のため永続 per-element マップをどのみち要求するので「状態ゼロ」の優位は相殺される。専用 `AccessIndex`（Decision 5）に置換。
- **run NodeId → ElementId を平坦な uniform グローバルマップで持つ** — 素直だが run 粒度のグローバル状態になり、span 消滅時の eviction を Parley マップと同期させる必要が出る。`AccessIndex` 名前空間への算術パック（Decision 5）で要素粒度の `Vec` 一本に畳めるため却下。
- **`SetTextSelection` を text-input 専用 `EditState` 経路に流す** — 既存構造に近いが、`EditState` は `cursor_byte_index` のみで範囲 anchor を持たず、a11y アクション層が要素種で分岐し意味論が裂ける。統一 Selection 着地（Decision 6）に置換。

## Consequences

- Core に inbound surface `on_accessibility_action` と Core enum `AccessibilityAction`、a11y サブシステムの `ElementId ⇄ AccessIndex` bimap が新設される。`Element` 構造体は a11y 非依存のまま。
- `AccessIndex` パッキングは「要素あたり TextRun 数 ≤ 2^k」を前提にする（k はパッキング実装時に確定）。`AccessIndex` 自体は要素数で有界で host id 幅に依存しない。要素削除時に bimap から `AccessIndex` を evict し、配下 run id を失効させる。
- 安定 NodeId のため各テキスト要素の `LayoutAccessibility` と run 採番カウンタはフレームをまたいで永続させる（毎フレームのカウンタ reset は再利用 id と新規 span の衝突を招くため禁止）。
- `SetTextSelection` と outbound `set_text_selection` 反映は Parley `LayoutAccessibility` 導入（text-run a11y）と同一作業単位で着地する。それまでの v1 は {Focus, Click/Default, ScrollIntoView, SetValue}。
- selection-change のアプリ向けイベントは引き続き MVP 範囲外（ADR-0097 と整合）。AT 向けの読み戻しは outbound TreeUpdate が担う。
- ネイティブ Platform Adapter crate が前提（PLAT-04 / PLAT-06、ADR-0087）。Web Canvas Mode の inbound は Safari EditContext 対応後に別 wire 拡張として設計する（ADR-0041）。
