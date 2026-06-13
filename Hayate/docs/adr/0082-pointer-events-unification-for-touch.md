# タッチ対応は Pointer Events への統一で行う

**Status: accepted**

現状 `attachPointerInput`（Web Adapter, ADR-0080により将来 `hayate-adapter-web` 自己配線へ移行）は `mousemove`/`mousedown`/`mouseup`/`wheel` のみを購読しており、タッチドラッグによるスクロールは一切処理されない。ADR-0046は「タッチジェスチャーの受信とスクロール物理（慣性・rubber-band）はPlatform Adapterの責務」と定義しているが未実装(open)である。

タッチ対応にあたり、Mouse Events群はそのまま残しTouch Eventsを別途追加する案と、Pointer Events（`pointerdown`/`pointermove`/`pointerup`/`pointercancel`、`event.pointerType`でmouse/touch/pen判別）に統一する案を比較し、**Pointer Events統一**を採用する。Mouse EventsとTouch Eventsを併用すると同一ジェスチャーに対し両方が発火する二重発火問題への対処が必要になり、Core側の `on_pointer_*` という命名（既にmouse/touch非依存）とも整合しないため。

タッチドラッグ→スクロールオフセット変換とスクロール物理（慣性・rubber-band）は ADR-0046 通り Platform Adapter（`hayate-adapter-web`）が担い、`element_set_scroll_offset`（SCR-02, プログラマティック専用API）経由でCoreに反映する。`touch-action: none` とPointer Captureの設定が必要。

## Consequences

- `mousemove`/`mousedown`/`mouseup`/`wheel` の購読は `pointerdown`/`pointermove`/`pointerup`/`pointercancel` + 既存`wheel`に置き換わる（ADR-0080の自己配線実装と合わせて移行）。
- ADR-0046で"open"だったタッチジェスチャー＋スクロール物理（慣性・rubber-band）の実装が必要になる。
- `on_pointer_down/up/move` のCore側シグネチャ自体は変更不要（座標ベースで既にpointerType非依存）。

## Amendment: `pointerleave` での hover 解除 (#212)

Canvas モードでは、ポインタが canvas サーフェスを出ても hover が解除されず `:hover` の見た目が残り続けるバグがあった（DOM モードはブラウザネイティブ `:hover` が leave を自動処理するため無縁）。原因は `pointermove`/`down`/`up`/`wheel` のみを自己配線し、`pointerleave` を購読していなかったこと。

これを正すため、Pointer Events 自己配線の対象に **`pointerleave`** を追加し、Core に座標非依存の `ElementTree::on_pointer_leave()` を新設する。`on_pointer_leave()` は内部の hover-clear 経路（`apply_pointer_hover(None)`）を呼び、hover 集合（hit 要素の祖先全体）を一括クリアして各 left 要素に `HoverLeave` を発火し、`last_pointer_pos` を reset する。**幻の `PointerMove` は生成しない**。HTML アダプタ既存の per-element leave seam と対称。

- `pointerleave` は座標非依存のため `toCanvas` 変換は不要。`pending_pointer` バッファには座標を持たない `PointerInput::Leave` として enqueue され、`render()` 冒頭の drain で arrival 順に `on_pointer_leave()` へ適用される。
- leave は 1px move-dedup の coalescing アンカーを reset する（Core が `last_pointer_pos` を None に戻すのと対称）。これにより leave 直後の同一座標への再 hover が drop されず `:hover` が確実に再適用される。

`render(timestamp_ms)` フレームループと backend（Vello / tiny-skia）には変更なし。hover 状態は Core で管理されるため backend 非依存で同一挙動。

## Amendment: `pointercancel` での hover ＋ `:active` 解除 (#213)

タッチデバイスでブラウザがポインタをキャンセルする（スクロール takeover や Pointer Capture 喪失）と、押下中のコントロールが `:active` のまま残り、永続的に押されたように見えるバグがあった。`pointerleave`（#212）は hover を解除するが、active な押下セッションは解除しない。

これを正すため、Pointer Events 自己配線の対象に **`pointercancel`** を追加し、Core に座標非依存の `ElementTree::on_pointer_cancel()` を新設する。`on_pointer_cancel()` は `on_pointer_leave()` と同じ hover-clear（`apply_pointer_hover(None)` + `last_pointer_pos` reset）を行い、**加えて** active な押下を終了する（`active_element.take()` → `ActiveEnd` 発火 + pseudo-activation dirty マーク）。これは既存の pointer-up 経路の active 終了処理を鏡写しにしたもの。**幻の `PointerMove` は生成しない**。

- `pointercancel` も座標非依存のため `toCanvas` 変換は不要。`pending_pointer` バッファには座標を持たない `PointerInput::Cancel` として enqueue され、`render()` 冒頭の drain で arrival 順に `on_pointer_cancel()` へ適用される。
- cancel は leave と同様に 1px move-dedup の coalescing アンカーを reset する（Core が `last_pointer_pos` を None に戻すのと対称）。これにより cancel 直後の同一座標への再 hover が drop されない。
- `pointerdown`/`up`/`move` の Core 側シグネチャは変更不要（座標ベースで pointerType 非依存のまま）。

self-wired な end-to-end 経路（実 DOM `pointermove`→`pointerleave` を headless browser で駆動し `poll_events()` で `HoverLeave` を assert）の回帰テスト戦略は ADR-0092 を参照。
