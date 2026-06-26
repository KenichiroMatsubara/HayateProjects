# §13 Mobile Capabilities

モバイル capability の公開境界と、wave-2 ストリーム型 capability の契約形。capability の三層
置き場（Core trait / Family Adapter / leaf）と scaffold 機構そのものは §9（PLAT-09 / PLAT-11）が
所有する。本章は capability を **Hayate を使うフレームワーク／アプリから見た契約面**として規定する。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### MOBL-01 — wave-2 ストリーム capability は `EventDelivery` とは別の専用契約
**規範文:** wave-2（battery / connectivity / geolocation / sensors）は「現在値の単発取得 ＋ 状態変化イベントの連続供給」が本質のストリーム型 capability で、`EventDelivery` / `DeliverySink`（element ターゲット＋bubble dispatch の DOM 風イベント・ADR-0117）には乗せない専用 Core 契約とする。各 capability は 1 trait に `query(&self) -> Result<T, CapabilityError>`（現在値・wave-1 同型）と `subscribe(&mut self) -> Result<Subscription, CapabilityError>`（変化ストリーム）の 2 メソッドを持ち、契約が保証するのは「`subscribe` が変化を流す」ことだけ（初期値は `query` 併用）。`Subscription` は RAII 購読ハンドルで、`poll_changes(&mut self) -> Vec<T>` を consumer がフレームの flush 点で drain し（`poll_deliveries` 同型・値コールバックは契約に持たない）、`Drop` で leaf が native 登録を解除する（解除は best-effort・`Result` を取らない）。値の届き方は wake = leaf push（`request_redraw`・ADR-0080/0117）／ value = consumer pull（drain）のハイブリッドで、threading marshaling とバッファは leaf に隠す。変化通知は ADR-0117 の「非同期 signal 変化」wake 源に合流し、tick の単一 flush 点で reactive flush する（App Host / `EventDelivery` は不変）。sensors は 1 trait ＋ `SensorKind` enum（高頻度サンプルは `Vec<T>` drain が落とさず吸収）、geolocation の権限は据え置き（`PermissionDenied` を今足さない）。置き場は wave-1 同型（Core trait ＋ `platform/mobile/` facade ＋ android/ios stub、stub は `query`/`subscribe` とも `Err(Unimplemented)`）。
**出典:** ADR-0120（ADR-0119 wave-2 / ADR-0117 / ADR-0080 / ADR-0003）
**状況:** ⬜ — 設計確定（ADR-0120）。後続の wave-2 scaffold issue（Core trait ＋ leaf stub ＋ facade）が着手可能。契約の最終形は実機実装で確定する（受容するリスク・ADR-0119 継続）。
**備考:** query/subscribe 分離・pollable drain・RAII subscription は `CONTEXT.md`「Stream Capability」「Capability Subscription」。wave-1 scaffold 機構は PLAT-11。

### MOBL-02 — capability の公開は Core trait の DI（in-process）。wire/JS 公開は延期し blocked issue で追跡
**規範文:** capability の framework 向け公開は新しい公開サーフェスを作らず、`Surface` / `FontFetcher` / `ImeBridge` と同型の **Core trait の依存性注入**で行う。App（合成ルート）が leaf facade（`MobileBattery` 等）を構築して consumer へ注入し、Element Layer（UI/描画の唯一の公開サーフェス）とは別軸の注入シームとして扱う。主 consumer は Hayabusa（in-process Rust・ADR-0045）で、`query`/`poll_changes` を Signal/Resource で reactive 化するのは Hayabusa 側の責務（Core 契約の外）。wire/JS 公開（`proto/spec/manifest.json` の mobile セクション投影）は不可避な収束先だが、wire 契約は in-process trait より桁違いに不可逆なため、wave-2 in-process 契約が実機実装で検証され、かつ wire/JS 需要（例: ADR-0121 の webview+wasm 経路）が現実化するまで延期し、別の blocked issue（#542）で追跡する。投影時は style_tags 同型の single-source codegen で行い JS 側で再宣言しない。
**出典:** ADR-0120（ADR-0068/0069 / ADR-0045 / ADR-0121）
**状況:** ⬜ — 設計確定（ADR-0120）。in-process DI は wave-2 scaffold と同時に成立。wire/JS 公開は未着手・blocked（発火条件まで proto/spec/manifest.json は不変）。
**備考:** 「Element Layer がただ一つの公開サーフェス」は UI/描画サーフェスの規範で、capability の注入シームと矛盾しない（Platform Adapter trait 群も元々 Element Layer ではない）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | – | — |
| 🟡部分 | – | — |
| ⬜未実装 | 2 | MOBL-01, MOBL-02 |
