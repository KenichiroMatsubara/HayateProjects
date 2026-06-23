# App Host の boot シーム：trait なしの `tick(timestamp_ms)` ＋ `request_redraw` コールバックループ、delivery は push 型 DeliverySink

status: accepted

Date: 2026-06-23

## Context

ADR-0068 がプラットフォーム非依存の Render Host / Font ロードを共有層へ hoist し、CONTEXT.md は
その共有層を **App Host**（tree 実体所有・フレームループ・Event Delivery drain・Font ロードを担い、
内部で Render Host を駆動する mount 先）として実体化・拡張した。Hayabusa（in-process Rust・ADR-0045）と
Tsubame Canvas Renderer（wire 経路）は共にこの App Host へ mount する。

App Host を実体化するにあたり、consumer（Hayabusa）と platform を繋ぐ二つの seam の形が未決だった。

1. **フレームループの所有と駆動シーム**：OS のフレーム駆動（web `requestAnimationFrame` / Android
   `Choreographer`）を誰が所有し、App Host にどう接続するか。App Host が consumer ごとの抽象を `trait`
   で受けるのか、もっと薄い接続にするのか。アニメーション（Transition・カーソル点滅・スクロール物理の
   `visual_dirty`）の継続フレーム要求を誰が出すか。
2. **Event Delivery のルーティングシーム**：CONTEXT.md は delivery drain を App Host の責務と定めた
   （Q3）。一方 `ListenerId` の正本は「host は `Map<ListenerId, handler>` だけを保持する」であり、
   in-process の Hayabusa では handler map は Hayabusa ランタイム側にある。App Host（drain 所有）から
   Hayabusa（handler map 所有）へ、drain した `{listener_id, event}` をどう渡すか。

App Host は consumer 非依存でなければならない（同一 App Host に Hayabusa も Tsubame Canvas Renderer も
mount する）。したがって両 seam とも「consumer 固有の知識を App Host に持ち込まない」形でなければならない。

## Decision

### フレームループ：trait なし。`tick(timestamp_ms)` ＋ 構築時注入の `request_redraw` コールバック

- **OS フレームループは Platform Front が所有する。** web binding が `requestAnimationFrame`、Android
  binding が `Choreographer` を回す。App Host は OS ループを所有しない。
- **App Host は `tick(timestamp_ms)` を公開する。** Platform Front が毎フレームこれを呼ぶ。`tick` は
  consumer 抽象を `trait` で受けない（per-consumer なフレームコールバック trait は導入しない）。
- **App Host は構築時に `request_redraw: impl Fn()` クロージャを Platform Front から受け取る。** `tick` の
  処理後にアニメーション／`visual_dirty` が残っていれば App Host が `request_redraw()` を呼んで次フレームを
  要求し、何も残っていなければ idle に落ちる（毎フレーム回し続けない）。継続フレームの判断は App Host、
  実際のスケジューリングは Platform Front。

`tick(timestamp_ms)` 1 回のフェーズ順序：

1. **drain**：App Host が `poll_deliveries()` を drain する（下記 DeliverySink へ）。
2. **handler ＋ flush**：consumer が handler 実行と reactive flush を行い、結果の Element Layer mutation を
   発行する（Hayabusa は in-process projection、Tsubame は wire projection）。
3. **commit_frame**：App Host が layout settling（`ElementTree::commit_frame()`）を行う。
4. **render**：`render_scene_graph` → Render Host → `Surface::present`。
5. **再要求判定**：アニメーション／`visual_dirty` が残れば `request_redraw()`。

### Event Delivery：push 型 DeliverySink を mount 時に登録

- **consumer は mount 時に `DeliverySink` コールバックを App Host に渡す。** App Host は drain を所有し続け
  （Q3 整合）、`tick` のフェーズ1で `poll_deliveries()` を drain し、得た delivery バッチを DeliverySink へ
  **同期 push** する。App Host は `ListenerId` の意味も handler の存在も知らない。
- **`ListenerId → handler` の map は consumer（Hayabusa ランタイムインスタンス）が所有する。** Hayabusa の
  DeliverySink は受け取った `{listener_id, event}` で map を引いて handler を実行し、reactive flush まで回して
  in-process で Element Layer mutation を発行し、**return する前にそのフレーム分の tree mutation を出し切る**。
  その後で App Host が commit_frame＋render へ進む（フェーズ2→3→4）。
- これは `ListenerId` 正本（host は `Map<ListenerId, handler>` だけを保持・bubble path は runtime 責務）と
  整合する。Tsubame Canvas Renderer では DeliverySink が wire を跨いで JS 側の map へ届く projection になる
  （現行 `poll_events()` pull モデルの後継として、App Host 主導の push に寄せる）。

## Considered Options

### フレームループ

- **per-consumer なフレームコールバック trait を App Host が受ける**：App Host が `trait FrameDriver` 等で
  consumer を呼ぶ。consumer ごとに trait 実装が要り、App Host が consumer 種別を意識する余地が生まれる。
  trait なしの `tick` ＋ `request_redraw` で同じことが薄くできるため却下。
- **App Host が OS ループ自体を所有する**：web/Android のフレーム駆動 API を App Host に持ち込むと
  platform 非依存性を壊す。Platform Front 所有・`tick` 注入を採用。
- **trait なし `tick(timestamp_ms)` ＋ `request_redraw` コールバック（採用）**：OS ループは Platform Front、
  継続判断は App Host、consumer 抽象は不要。最も薄い seam。

### Event Delivery

- **pull：consumer が `poll_deliveries()` を自分で引く**：App Host は drain せず `tick` 内で
  `consumer.update(ts)` を呼び、consumer が自ら drain する。現行 Tsubame の `poll_events()` pull と同型だが、
  CONTEXT.md が定めた「drain は App Host 所有」（Q3）が consumer 側へ漏れる。却下。
- **push：DeliverySink コールバック（採用）**：App Host が drain を所有し続け、mount 時登録の sink へ同期
  push する。consumer は `ListenerId → handler` map と handler 実行・flush だけを持つ。App Host は consumer
  非依存を保つ。

## Consequences

- App Host の公開 surface が `tick(timestamp_ms)` と、構築時の `request_redraw` 受け取り、mount 時の
  `DeliverySink` 受け取りに定まる。consumer 向けのフレーム trait は導入しない。
- アニメーション継続フレームの責務が明確化：App Host が `visual_dirty`／進行中 Transition／スクロール物理を
  見て `request_redraw()` を出す。Platform Front は OS ループの配管に徹する。
- Hayabusa は Platform Front を直接触らず（Platform Adapter も直接触らない・CONTEXT.md）、App Host へ
  root・`DeliverySink` を渡して mount するだけになる。`ListenerId → handler` map は Hayabusa ランタイムが持つ。
- Tsubame Canvas Renderer の現行 `poll_events()` pull は、App Host 主導の push DeliverySink（wire projection）
  へ寄せる後継方向となる。raw event ではなく drain 済み `{listener_id, event}` を運ぶ点は不変。
- **受容するリスク**：DeliverySink を in-process（Hayabusa）先行で確定するため、wire（Tsubame）projection で
  形の手戻りがありうる。`poll_events()` の `{listener_id, event}` 形をそのまま push に載せ替えるだけに保ち、
  露出を抑える。

## 関係

- ADR-0068：プラットフォーム非依存の Render Host / Font ロードを共有層へ hoist。本 ADR はその共有層を
  App Host として実体化し、boot（loop / delivery）seam を定める。
- ADR-0045：Hayabusa は hayate-core に Rust crate 直リンク（WIT なし）。本 ADR の in-process Element Layer
  projection / DeliverySink in-process 経路はこの直リンク上に乗る。
- ADR-0018：export poll モデル。Event Delivery（`poll_events()`）の後継として App Host 主導の push へ寄せる。
- ADR-0080：ネイティブ提供イベントは Platform Adapter が購読・変換まで完結。Platform Front（OS フレーム
  ループ）と Platform Adapter（Surface/FontFetcher/IME/input）は別軸として並立する。
