# transition のトリガは effective style 解決シームの差分（Blink 準拠の up-level、ADR-0089 のスコープを更新）

**Status: accepted（HITL — grill session 2026-06-13）。実装は後続スライス。**

**Date: 2026-06-13**

> 本 ADR は ADR-0089 の「トリガ範囲・`from` 捕捉・state 粒度」を更新（supersede）する。
> ADR-0089 のその他の決定（Render Layer 補間・frame loop 再利用・scene_build blend・
> 補間対象6連続値・enum 即時・DOM 委譲）はそのまま据え置く。受理時点でコードは
> ADR-0089 の pseudo-only 実装のままであり、本決定の実装は後続。

## Context

ADR-0089 は補間トリガを擬似状態切替（`mark_pseudo_activation_dirty` 単一シーム）に
限定し、`setStyle` 直接 mutation は即時とした。これは**性能制約ではなく**、実装コストの
非対称が理由 — 擬似状態切替は `focus`/`hover`/`active` の全経路が既存の単一シームに集約
済みで、そこに1行足すだけで「`from` 捕捉＋transition 開始」が揃う。一方 `setStyle`
（`apply_mutations` 経路）は別経路で `from` 捕捉機構を持たないため、補間させるには新機構が
要る。

結果、DOM 経路との Semantics Parity（system-wide ADR-0002）が割れていた:

- DOM 写像（generated `dom_style_mapper.rs` / Tsubame）は `transition-duration` /
  `transition-timing-function` を CSS に出すが `transition-property` を**出さない**＝CSS
  既定 `all`。ブラウザは `setStyle` 由来を含む**あらゆる算出値変化**を補間する。
- よって `transition-duration` を持つ要素に `setStyle` で色を変えると **Canvas=即時 /
  DOM=補間** で割れる。逆方向割り込みも **Canvas=ジャンプ（`from` が解決値）/ DOM=連続**
  で割れる。

Blink（CSS Transitions 仕様）は入力イベントではなく **before-change style と
after-change style の per-property 差分**でトリガし、before-change style は**実行中の
アニメーションを現在時刻まで進めた値**を含む。これが「原因非依存の発火」「連続反転」
「プロパティ独立」を同時に生む。Hayate には既に effective style を解決する単一シーム
（`resolve_effective`・ADR-0067）があり、Blink の「style recalc 中の transition 更新」と
同じ位置に補間トリガを置ける。

## Decision

up-level でパリティを取る。Blink と同型に倒す。

1. **トリガを `resolve_effective` 単一シーム（ADR-0067）の差分へ移す。** pseudo trigger
   への hook を廃し、dirty 要素ごとに「前フレーム表示の effective visual（blend 込み）」
   vs「新たに `resolve_effective` した値」を連続値プロパティ単位で diff する。差があり
   解決済み `transition-duration > 0` なら transition を開始/調整する。擬似状態切替・
   `setStyle`・継承変化（ADR-0065）・viewport variant（ADR-0081）を区別しない。

2. **`from` は現表示値（blend 込み）を使う。** ADR-0089 の「`from` = 解決済み pre-switch
   値」を更新し、逆方向割り込みを連続反転にする（Blink の before-change style と同型）。

3. **state は要素×プロパティ単位。** ADR-0089 の「1要素1 `TransitionState`」を更新する。

4. **DOM 側は変更不要。** `transition-property` を出さない現状（既定 `all`）が up-level
   方針と一致する。Canvas を DOM の挙動へ寄せる形でパリティが取れる。

## Consequences

- 差分表の parity 破れ（トリガ範囲 / setStyle / 対象プロパティ範囲 / 逆方向）が解消。
  `setStyle` も継承変化も両経路で補間され、Web のメンタルモデルに一致する。
- コストは「全 dirty 要素で毎フレーム diff」へ増える（Blink のメインスレッド transition
  更新相当）。frame loop / blend / scene_build 経路は既存のまま。
- **reversing shortening factor は入れない（既知の単純化）。** Blink は反転時に残り進捗比で
  duration を縮めるが、本決定は単純連続反転に留める。必要になれば別スライスで深める。
- border-width の layout 結合差は transition 固有でなく box モデルのパリティ問題のため、
  本 ADR の対象外。

## 実装方針（before-change の保持と発火点）

- **before-change（前フレーム表示値）の保持先**: retained 描画状態（`SceneLowering.anchors`
  の `AnchorEntry`）に、表示中（post-blend）の `Visual` を memo する。lifecycle が要素の
  寿命と一致し、要素削除時に anchor と同じ経路で掃除されるため。これは派生値のキャッシュ
  であり第二の正本ではない（ADR-0057 非抵触。既に `TransitionState.from` が Visual
  スナップショットを ElementId キーで持つのと同種）。`tree.transitions` への相乗りは
  「進行中だけ」という寿命差から不適として却下。
- **発火点**: `emit_element`（scene_build）の `resolve_effective` 直後に diff する
  （resolve を二度引かない）。順序は resolve →（cached `last_visual` と per-property diff、
  差があり `transition-duration > 0` なら from=cached で transition 開始/調整）→ blend →
  lower → 新しい post-blend を `last_visual` へ格納。
- **初回 emit は transition しない**: cache（before）が無いため。Blink の「初期スタイルは
  transition しない」と一致し、要素生成時の意図しないフェードインを防ぐ。
- **retained incremental 経路への依存**: 補間は ADR-0086 の retained anchor 経路で成立する。
  full ephemeral rebuild（`walk_ephemeral`、parity 参照 / テスト用）は `last_visual` を
  持たず補間しない。「ephemeral はパリティ参照でありアニメーションしない」と割り切る。
- **開始時刻（per-property、明示グループ無し）**: state は要素×プロパティ単位で、各々
  `start_ms` を「切替後最初の render」で遅延アンカーする（ADR-0089 の遅延方式を踏襲）。
  同フレームで変わった複数プロパティは次の render で一斉に advance されるため**結果的に
  同一 `start_ms` を共有**し、別フレーム変更は自然に別 start になる。明示的な transition
  group は導入しない。`advance_transitions(now_ms)` が単一 `now_ms` を配る前提に依存する。
- **duration / timing は after-change（resolve 済み effective visual）から読む**: CSS の
  「開始する transition の `transition-*` は after-change style を使う」規定に合わせる。
  diff 地点では resolve 済みの `visual`（pseudo / viewport / 継承 反映済み・blend 前）が
  手元にあるので、その `transition_duration` / `transition_timing` をそのまま使う。in/out で
  duration が非対称になり得る（例: `:hover { transition-duration: 0 }` → 即時 in・base の
  duration で out）のが CSS/DOM 準拠の正しい挙動。現コードの `el.visual`（base）直読は
  pseudo 上書きを無視する潜在バグであり、本決定で廃止する。

## Considered Options

- **Down-level（DOM 側で `setStyle` 時に transition 抑止）**: パリティは安価に回復するが、
  「effective が変われば補間」という Web 標準のメンタルモデルから外れ、`setStyle` アニメを
  将来欲しくなった時に作り直しになる。却下。
- **現状維持（pseudo-only ＋ 乖離を ADR に明示するだけ）**: 沈黙の経路差が残り system-wide
  ADR-0002 に反する。却下。
