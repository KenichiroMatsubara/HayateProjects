# wave-2 ストリーム capability の契約形：`EventDelivery` とは別の専用契約・query/subscribe 分離・pollable drain・RAII 購読ハンドル

status: accepted

Date: 2026-06-26

## Context

ADR-0119 が mobile capability を wave 分割し、wave-2（battery・connectivity・geolocation・sensors）を
「**query ＋ 状態変化イベントの連続供給**」が本質のストリーム型として wave-1（一発応答）から切り出し、
契約形の設計を後回しにした。今その契約形を決める（後続 scaffold issue の blocker 解除）。

決めること（issue #518）は 3 つ:

1. ストリームを既存のイベント経路（`EventDelivery` / `poll_deliveries` / `DeliverySink`・ADR-0117）に
   乗せるか、専用契約にするか。
2. query（現在値の単発取得）と event（変化通知）の分離の仕方。
3. 解約（unsubscribe）／ライフサイクルの所有者。

前提となる既存決定:

- `EventDelivery { listener_id, event }` は **element ターゲット＋bubble dispatch の DOM 風イベント**
  （`event_target()` が必ず `ElementId` を返す）。App Host が `poll_deliveries()` で drain し `DeliverySink`
  で consumer へ push、**flush 点は tick フェーズ2の 1 箇所**（ADR-0117）。
- ADR-0117 は idle からの wake 源を 3 つと定める: 継続（App Host）／入力到着（Platform Adapter）／
  **非同期 signal 変化（consumer）**。`Resource` 解決・`Store` 外部更新・timer がこの 3 番目。
- ADR-0080: native が自動供給するイベントは Platform Adapter が購読・変換まで完結する。
- ADR-0003: Core は単一スレッド。ADR-0119: 契約は Core 所有・実装は leaf・`Result<T, CapabilityError>`・
  stub は `Err(Unimplemented)`・panic 禁止（FFI 越え abort 回避）・`platform/mobile` の cfg facade。

## Decision

### 1. 専用契約。`EventDelivery` 経路には乗せない。フレーム/flush 規律だけ再利用

- ストリーム capability は **Core 所有の専用 trait**として定義し、`EventDelivery` には乗せない。
  capability ストリームは **アプリ全体のデバイス状態**で element も hit-test も bubbling も無い。
  `EventDelivery` に乗せると合成 element／偽 listener という category error になり、`ListenerId` 正本
  （host は element 由来 handler map だけ）も汚れる。
- ただし**出口の規律は再利用**する。capability の状態変化は ADR-0117 が既に定義済みの
  **「非同期 signal 変化」wake 源そのもの**。「native source →（leaf がバッファ）→ `request_redraw()`
  → tick の単一 flush 点で reactive flush → mutation」に合流させ、**App Host と `EventDelivery` は
  一切触らない**（App Host は consumer 非依存のまま）。

### 2. capability ごと 1 trait、`query()` ＋ `subscribe()` の 2 メソッド。契約は「変化を流す」のみ保証

- capability ごとに 1 trait。`query(&self) -> Result<T, CapabilityError>`（現在値・wave-1 と同型）と
  `subscribe(&mut self) -> Result<Subscription, CapabilityError>`（変化ストリーム）の 2 メソッドを持つ。
- 契約が保証するのは **`subscribe` が変化を流すこと**だけ。初期スナップショットが要る consumer は
  `query()` を別に呼ぶ（Flutter `batteryLevel` vs `onBatteryStateChanged`、`getCurrentPosition` vs
  `watchPosition` の prior art に倣う）。「`subscribe` が最初に現在値を 1 回出すか」は capability ごとの
  実装裁量で、契約には載せない。

### 3. 値は pollable drain（`Vec<T>`）。値コールバックは契約に持たせない。wake は leaf、value は consumer pull

- `subscribe` が返す `Subscription` は **pollable**: `poll_changes(&mut self) -> Vec<T>` で蓄積された変化を
  consumer が**フレームの flush 点で drain** する（`poll_deliveries` と同型）。Core 契約に値コールバック
  （`FnMut(T)`）は置かない。
- **threading marshaling とバッファリングは leaf（Platform Adapter）の中に隠す**。native callback は
  platform スレッドで発火する（Android `SensorManager` 等）が、単一スレッド Core（ADR-0003）へは
  leaf がバッファ経由で渡す。consumer は自スレッドの flush 点で `Vec<T>` を引くだけ。
- **idle を壊さないための wake**（`request_redraw`）は **native ingress 側＝Platform Adapter（leaf）が
  叩く**。これは ADR-0080（native 自動供給イベントは Platform Adapter）＋ADR-0117（入力到着＝Platform
  Adapter が wake 源）に乗る。つまり **wake = leaf push（軽い通知）／ value = consumer pull（flush 点で
  drain）** のハイブリッド。pure poll（毎フレーム引きに行く）は idle 落ち（ADR-0117）を壊すので採らない。

### 4. RAII 購読ハンドル。consumer 所有、`Drop` で leaf が native 解除、解除は best-effort

- `Subscription` は **RAII ハンドル**で、購読の生存そのもの。明示 `unsubscribe(id)` は設けない。
- **所有者は consumer（アプリ／Hayabusa ランタイム側）**。component の unmount でハンドルが drop し
  購読も終わる。手動ペアの呼び忘れによる native listener／sensor のリーク（電池消費直結）を型で防ぐ。
- `Drop` が leaf へ native 登録の解除を伝える（**契約と Drop 意味論は Core、解除の native 手続きは leaf**）。
  `Drop` は値を返せないため**解除失敗は best-effort で握り潰す**（解除に `Result` を取らない）。
- 多重購読は「ハンドル 1 つ = 購読 1 つ」だけを契約し、native 登録の集約／参照カウントは leaf 裁量。

### 5. 4 capability の適合：battery/connectivity/geolocation は個別 trait、sensors は 1 trait ＋ kind enum

- **battery**: `query() -> BatteryStatus`（level・charging）＋ `subscribe`。
- **connectivity**: `query() -> Connectivity`（wifi/cellular/none 等）＋ `subscribe`。
- **geolocation**: `query() -> Position`（`getCurrentPosition` 同型）＋ `subscribe`（`watchPosition` 同型）。
  **権限は据え置き**: ADR-0119 が permissions を「さらに後」に倒しているため、scaffold は `Err(Unimplemented)`
  のまま、`PermissionDenied` variant も**いま足さない**（実機実装時に追加）。
- **sensors**: **1 trait ＋ `SensorKind` enum**（Accelerometer/Gyroscope/Magnetometer…）。各 sensor は契約形
  （3軸 `f64` ＋ timestamp の `SensorSample`）が同型で kind 分岐のみ。Flutter `sensors_plus` の prior art。
- **`Vec<T>` drain が高頻度 sensor を吸収**: 200Hz × 60fps で毎フレーム ~3 サンプルを**落とさず全部渡し**、
  coalesce は consumer 裁量。battery/connectivity の「最新状態だけ欲しい」離散遷移も同じ `Vec` で表せる
  （最後だけ見る）。1 つの形が両極を吸収する（決定 3 の裏打ち）。
- 値型（`BatteryStatus`/`Connectivity`/`Position`/`SensorSample`）は **common 部分集合の seed** とし、
  platform 固有フィールドは実機実装時に拡張（`DeviceInfo` の common 部分集合と同じ流儀・ADR-0119）。

### 6. 公開境界：in-process は Core trait の DI で公開。wire/JS 公開は不可避な収束先だが今は延期し blocked issue で追跡

- **公開形は新サーフェスを作らず、Core trait の依存性注入**。capability trait は `Surface` / `FontFetcher`
  / `ImeBridge` と同型の注入シームで、App（合成ルート）が leaf facade（`MobileBattery` 等）を構築して
  consumer へ注入する。**Element Layer とは別軸**（「Element Layer がただ一つの公開サーフェス」は UI/描画
  サーフェスの話で矛盾しない）。capability 用の新「公開サーフェス」概念は作らない。
- **主 consumer は Hayabusa（in-process Rust・ADR-0045）**。`query`/`poll_changes` を Signal/Resource で
  reactive 化するのは **Hayabusa 側の責務で Core 契約の外**。Core 契約は最小の trait のまま保つ。
- 散文 spec に **§13「Mobile Capabilities」**を新設して capability 面を文書化する（reversible な文書化）。
- **wire/JS 公開（`proto/spec/manifest.json` の mobile セクション投影）は不可避な収束先**だが、wire 契約は
  バージョン付き・言語横断・JS consumer が依存する **in-process trait より桁違いに不可逆**。実機実装が
  1 つも無く streaming 契約（`poll_changes`/`Vec<T>`/RAII）が未検証の段階で焼くのは ADR-0068 の投機 seam の
  罠。よって**今は manifest を触らず**、wire 公開は **別の blocked issue で追跡**する。発火条件は
  「wave-2 in-process 契約が実機実装で検証済み」かつ「wire/JS 需要が現実化（例: ADR-0121 の webview+wasm
  経路、または JS フレームワークからの capability 要求）」。投影は style_tags 同型の single-source codegen で
  行い、JS 側で再宣言しない。

### 7. 置き場：wave-1 と同型（Core trait ＋ `platform/mobile/` facade ＋ android/ios stub）

- wave-2 も **Core trait ＋ `platform/mobile/` の cfg facade（`MobileBattery`/`MobileConnectivity`/
  `MobileGeolocation`/`MobileSensors`）＋ android/ios stub**。stub は `query`/`subscribe` とも
  `Err(Unimplemented)` を返し panic しない。host テストで両 platform 名を assert（wave-1 と対称）。

## Considered Options

- **`EventDelivery` を非 element イベントも運べるよう拡張**: 既存経路を再利用できるが、element/bubble 前提の
  正本（`ListenerId`・`event_target`）を曲げ、App Host を capability-aware にする。category error。却下。
- **純コールバック `subscribe(callback: FnMut(T))`**: 簡潔だが `Send`／再入／closure 保持を実装前に確定させ、
  ADR-0119 の「実装で契約形が変わる」リスクに直撃。pollable drain なら threading を leaf に隠せる。却下。
- **pure poll（wake なし・毎フレーム drain）**: 単純だが idle 落ち（ADR-0117）を壊す。leaf-wake ハイブリッド採用。
- **明示 `subscribe`/`unsubscribe(id)` ペア**: 呼び忘れで native listener がリーク。RAII で型保証。却下。
- **sensors を per-sensor trait に分割**: 契約形が同型なので trait 増殖は facade を散らすだけ。kind enum 採用。
- **いま wire（proto/spec mobile セクション）まで設計・公開**: (b) は収束先だが未検証契約の不可逆な前倒し。
  in-process 先行・wire は blocked issue で追跡に倒す。

## Consequences

- wave-2 の各 capability が「Core trait（`query` ＋ `subscribe`）＋ `Subscription`（RAII・`poll_changes`）＋
  android/ios stub ＋ `platform/mobile` facade」で scaffold 可能になり、後続 scaffold issue の blocker が解除。
- `EventDelivery` / `DeliverySink` / App Host は**不変**。capability ストリームは「非同期 signal 変化」wake 源
  として既存の単一 flush 点に合流し、新しい out-of-band な flush 経路は作らない。
- `CapabilityError` に variant 追加は無し（`PermissionDenied` は最初の権限ゲート capability 実機実装時）。
- 散文 spec に §13「Mobile Capabilities」を新設、`CONTEXT.md` に Stream Capability / Capability Subscription を
  追加。`proto/spec/manifest.json` は不変。
- **受容するリスク**: 契約形（`Subscription` のバッファ機構・値型の common 部分集合）は実機実装で変わりうる
  （ADR-0119 の受容リスク継続）。wire 公開を遅らせることで、変更コストを in-process 範囲に閉じ込める。

## 関係

- ADR-0119（mobile capability breadth-first scaffold）：本 ADR はその wave-2（保留していたストリーム契約）を
  確定する。契約 Core 所有・実装 leaf・`Result`・stub `Err(Unimplemented)`・cfg facade を継承。
- ADR-0117（App Host boot seam・三層モデル）：`EventDelivery`/`DeliverySink`/単一 flush 点/wake 源 3 種を
  再利用しつつ、capability ストリームを「非同期 signal 変化」源として合流させる（App Host は不変）。
- ADR-0080（native 自動供給イベントは Platform Adapter）：capability の native ingress と wake を Platform
  Adapter（leaf）が所有する根拠。
- ADR-0068/0069（契約は Core）：capability trait・`Subscription` 契約・Drop 意味論を Core 所有とする型を継続。
- ADR-0003（単一スレッド Core）：値コールバックを避け pollable drain にする根拠（marshaling を leaf に隠す）。
- ADR-0121（WebView+wasm ニア・ネイティブ経路）：wire/JS 公開（決定 6 の延期分）を将来発火させるトリガ候補。
