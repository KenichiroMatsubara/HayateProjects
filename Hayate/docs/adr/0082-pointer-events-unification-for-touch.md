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

## Amendment: Canvas Mode タッチドラッグ・スクロール物理の実装方針

本ADR本文と ADR-0046 で "open" としていた「タッチドラッグ→スクロールオフセット変換＋スクロール物理（慣性・rubber-band）」を、Canvas Mode（`hayate-adapter-web` の `CanvasRenderer`）について以下の方針で確定する。**DOM Mode は scroll-view が `overflow:auto` の実 div でブラウザネイティブのタッチスクロールが効くため対象外**。

### offset の適用経路

スクロールオフセットは `ElementTree::element_set_scroll_offset`（SCR-02, clamp なし）で適用する。`apply_wheel_delta` は内部で `[0, max]` に clamp するため **overscroll を表現できず rubber-band に使えない**ので採用しない。overscroll は scroll group の transform `[1,0,0,1,-sx,-sy]` が scroll-view の Clip 内で成立し、端からコンテンツが離れるネイティブ同等の見た目になる（`scene_build`）。Core 側に新 API は追加しない。

### アニメーション駆動

Canvas Mode の host は連続 rAF ループで毎フレーム `render(timestamp_ms)` を呼ぶ（`@tsubame/renderer-canvas` `CanvasRenderer.frame` が自己再スケジュール）。慣性・spring-back は **この `render(timestamp_ms)` 内でフレーム間 dt を用いて積分**する。新規のフレーム駆動機構は追加しない。物理状態（active scroll-view・速度・overscroll 状態）は `CanvasRenderer`（Adapter）が所有する。

### 入力分類とジェスチャモデル

- **対象ポインタ**: `touch` と `pen` のみドラッグ→スクロール経路に乗せる。`mouse` は現状維持（選択・ドラッグ）。判別のため `PointerInput` に `pointerType` をスレッドする。`wheel` 経路は不変。
- **タップ/スクロール判別**: `pointerdown` で一旦 `on_pointer_down` を Core に送り（タップで `:active` 表示）、移動量がスロップ閾値（≈8px）を超えたらスクロールと判定し `on_pointer_cancel()`（#213）で押下を解除、以降の move はスクロールに振り向ける。閾値未満で離せば通常の `pointerup`→click。
- **主ポインタのみ**: `isPrimary` の 1 本のみ追跡し、2 本目以降と pinch は無視（マルチタッチ/ズームは v1 スコープ外）。
- **ジェスチャロック**: `pointerdown` のヒット要素の最近接 scroll-view 祖先にジェスチャをロックし、ジェスチャ途中で祖先へスクロールチェーンしない。ロックした scroll-view が自身の端で rubber-band する。

### scroll イベント通知

タッチ駆動のスクロール中も `Event::Scroll` を発火する（ネイティブ同等。parallax/lazy-load がタッチでも機能）。offset 適用（`element_set_scroll_offset`）と通知（`ElementTree::on_wheel` 相当の scroll-notify シーム）は別経路として明確に分離する。ADR-0046 の「`scroll` イベントはアプリ通知専用、`element_set_scroll_offset` はオフセット機構」という分担と整合する。

### `touch-action` / Pointer Capture

`touch-action: none` と `setPointerCapture`（ジェスチャ中、指が canvas 外へ出ても move を受け続ける）は ADR-0080 の自己配線方針に従い **Rust adapter が canvas に対して設定**する。host/`index.html` 側の glue は追加しない。

### テスト戦略

スクロール物理は純関数（入力: 前 offset・速度・dt・bounds → 出力: 新 offset・新速度。慣性減衰・rubber-band 抵抗・spring-back）として切り出し、全ターゲットでユニットテストする（`coalesce_pointer_inputs` と同パターン）。wasm 配線は薄く保ち、self-wired な headless browser 回帰を 1 本（ADR-0092）追加する。物理係数（摩擦・ばね減衰）は iOS 風デフォルトで実装時に調整する。

### Pending（本Amendment時点で未確定・スコープ外・将来課題）

本Amendmentは**方針の確定**であり、以下は未着手・未決定として明記する。

- **実装そのものが未着手**: 上記方針に基づく `pointer_input.rs`（`pointerType` スレッド・`touch-action:none`・`setPointerCapture` 配線）／`CanvasRenderer`（物理 state・`drain_pointer_inputs` の touch 分岐・`render()` 内積分）／物理純関数モジュール＋テスト／WASM 再ビルド／headless e2e は本Amendment時点で未実装。
- **物理係数の具体値は未確定**: 摩擦係数・rubber-band 抵抗カーブ・spring-back 剛性/減衰の具体値は実装時に iOS 風デフォルトから調整して決める（本Amendmentでは固定しない）。
- **マルチタッチ / pinch-zoom はスコープ外**: v1 は `isPrimary` 1 本のみ。2 本指ジェスチャ・ピンチズームは将来課題。
- **ネストした scroll-view のスクロールチェーンは将来課題**: v1 は `pointerdown` 時の scroll-view にロックし、端到達時の祖先チェーンは行わない。タッチでのチェーン（rubber-band/慣性を含む受け渡し）の設計は別途。
- **`scroll-snap-type` / `scroll-snap-align`（ADR-0046）は未実装**: スナップ・ページネーションは本Amendmentの対象外。
- **scroll-notify シームの命名整理は保留**: タッチ通知に `ElementTree::on_wheel`（実体は `Event::Scroll` 発火）を再利用するか、`notify_scroll` 等へ抽出・改名するかは実装時の小リファクタ判断として保留。
- **DOM Mode の Core `scroll_offset` 同期は対象外**: DOM Mode はネイティブスクロールで視覚上は動くが、ネイティブ `scroll` での Core `scroll_offset` 同期は本Amendmentでは扱わない（別課題）。
