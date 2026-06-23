# Pending Decisions（未決 ADR 論点）

> デモアプリ（画面に出て触れるレベル）に到達するために決める必要があるが、まだ ADR に
> なっていない論点の記録。**ロジック層（リアクティブ／テンプレート／コンポーネント／式／
> async モデル）は ADR 0001–0006 で決定済み・実装も追従済み**で、ここに挙げる穴はすべて
> **Hayate（画面・入力・スタイル）との境界**にある。いずれも新しいリアクティビティを必要と
> せず、既存コアのまま乗せられる。
>
> ステータス凡例：🔴 critical（デモの前提）／🟠 high（見せられるデモに必要）／
> 🟡 medium（あると良い・初回デモには必須でない）。
>
> 各項目が ADR 化されたら、その旨を追記して `docs/adr/` に移すこと。

## 現状サマリ

| 領域 | 根拠 ADR | 状態 |
| ---- | -------- | ---- |
| リアクティブコア（Signal/Memo/Effect・所有 Scope） | 0003 | ✅ 実装済 |
| Template IR・binding・構造 reconcile（`:if`/`:each`） | 0004 | ✅ 実装済 |
| コンポーネント（prop/emit/lifecycle） | 0004 | ✅ 実装済 |
| 式 DSL パーサ | 0004 | ✅ 実装済 |
| async/Resource/Suspense/ErrorBoundary | 0005 | ✅ モデル決定済（実装は未） |
| host-ABI・モノレポ配置・hot-reload | 0001/0002/0006 | ✅ 方針決定済 |
| レンダリング統合・boot・フレームループ・イベント配送 | 0117 | ✅ 方針決定済（実装は未） |

---

## P1 🔴 レンダリング統合とアプリ起動

> **更新（2026-06-23）**：本項の設計面は **ADR-0117（App Host boot シーム）で決着**した。
> 残る critical は **ビルド spike 一点**（下記）。設計の決着内容：
>
> - **ElementTree の所有位置** → App Host が `ElementTree` 実体を所有。Hayabusa は in-process
>   projection で `&mut ElementTree` に直接 mutation を発行（wire 非経由・ADR-0045 / CONTEXT.md）。
> - **アダプタ方針** → Hayabusa 専用アダプタは持たない。共有 App Host へ `mount(root, DeliverySink)`
>   するだけで、Platform Front（web `requestAnimationFrame` / Android `Choreographer`）と
>   Platform Adapter（GPU/DOM 描画・入力・IME・ADR-0080）を再利用する。Hayabusa は Platform Front /
>   Adapter を直接触らない。
> - **boot / フレームループ** → App Host が `tick(timestamp_ms)` と構築時注入の `request_redraw`
>   クロージャで所有。OS ループは Platform Front 所有。継続フレーム要求（transition・カーソル点滅・
>   スクロール物理）は App Host が `visual_dirty` を見て出す。consumer 向けフレーム trait は無し。

**残る未決（実装ブロッカー・spike 対象）**：hayate-core は vendored crate を `[patch.crates-io]`
（Hayate ワークスペース）で差し替えており、別ワークスペースの Hayabusa から path 依存でリンクすると
patch が効かない可能性。wasm 同梱・パッケージングも未検証。**ADR-0117 はこのビルド現実に触れていない。**

**現在の代替**：`ElementSink`（`src/sink.rs`）が ElementTree の API に 1:1 で写るシームに
なっており、テストは `RecordingSink` で fine-grained patch を観測している。App Host が渡す
`&mut ElementTree` を駆動する `HayateSink`（DeliverySink 実装込み）は薄い後続実装。

**ADR にすべきこと**：設計は ADR-0117 で済み。spike の結果クロスワークスペース構成に固有の決定
（クレート配置・patch 解決・wasm パッケージング）が必要なら、それを ADR 化する。

**推奨**：**spike を先に行う**（hayate-core を path 依存で Hayabusa から実際にビルドできるか
＝クロスワークスペース問題の検証）。spike が通れば残りは実装タスクで、追加 ADR は不要な見込み。

## P2 🟢 イベント入力の経路 — **ADR-0117 で決着**

**決着**：App Host が `tick` フェーズ1で `poll_deliveries()` を drain し、フェーズ2で mount 時登録の
`DeliverySink` へ drain 済み `{listener_id, event}` バッチを同期 push する。Hayabusa の DeliverySink は
自身が所有する `ListenerId → handler` map を引いて handler を実行し、reactive graph を flush して
（handler 由来・非同期由来とも flush 点はこの 1 箇所）、in-process で `&mut ElementTree` に mutation を
出し切ってから return する。App Host は `ListenerId` の意味も handler の存在も知らない（consumer 非依存）。
テンプレの `on:click` / `on:input` は handler を ListenerId に紐付けて map に登録するだけ。

**残るのは実装のみ**：テスト用シーム `Instance::click(ElId)`（`src/instantiate.rs`）を、実機では
DeliverySink 経由の `{listener_id, event}` dispatch に置き換える。新しい ADR は不要。

## P3 🟠 スタイル（`<style>` DSL → Hayate CSS）

**問題**：CONTEXT.md は「`<style>` は言語非依存の DSL」「Hayate CSS は要素ローカルの
インラインスタイル」と定義するだけ。未決は次の通り。

- オーサリング面（プロパティ集合・単位・`:hover`/`:active`/`:focus` の素通し）。
- スコープ（コンポーネント単位の scoped style を持つか）。

加えて、**sink / Template IR に現状 style オペが無い**（`set_style` 等の拡張が要る）。
これが無いとレイアウト・色が出ず「見せられるデモ」にならない。

> **決定（2026-06-23）：reactive なスタイル束縛は一旦禁止。** 初回デモは **static style のみ**
> とし、`{expr}` 駆動の style プロパティ束縛も条件付きクラス相当も載せない。binding 機構は
> 触らず、sink に「静的 style を一度セットする」オペ（`set_style`）を足すだけにする。reactive
> style が必要になった段階で別途 ADR 化する（既存 binding 機構に乗る見込みなので低リスク）。

**ADR にすべきこと**：「Hayabusa スタイル DSL（static）と sink/IR の `set_style` 拡張」。

## P4 🟠 フォーム／`text-input` の双方向束縛

**問題**：controlled input の `value` 束縛、`on:input` / `on:submit`、EditIntent 統合
（Hayate 側は EditState/EditIntent を所有）。Todo 系デモに必須だが ADR 無し。controlled vs
uncontrolled の意味論も未決。

**ADR にすべきこと**：「入力 / value 束縛の意味論」（P3 の一部にしても良い）。

## P5 🟡 Store（コンポーネント横断状態）

**問題**：CONTEXT.md に Store 語彙はあるが ADR 無し。非自明なデモは共有状態を欲しがる。
ADR-0003 の Scope 階層に乗る provide/inject で小さく実装できる見込み。Router は語彙のみで
ADR 無し（初回デモには通常不要）。

**ADR にすべきこと**：「Store の provide/inject と Scope への載せ方」。

## P6 🟡 `.hybs` ファイル＋`<script>` コンパイル

**問題**：`<template>` / `<style>` マークアップのパーサは ADR-0004＋CONTEXT から導出可
（低リスク）。難所は **Rust-native `<script>` のコンパイル/登録経路**（ADR-0001 は「境界ゼロ」
と言うが具体 ADR 無し）。

**注**：デモに `.hybs` は必須でない。テスト同様に Template IR ＋ Rust ハンドラを手組みすれば
書ける。`.hybs` オーサリングまで欲しくなった段階で必要になる論点。

---

## デモ到達への最短経路（メモ）

1. **P1 の spike**（クロスワークスペース・ビルド可否の検証）。通れば P1 は実装タスク化。
2. **`HayateSink` ＋ DeliverySink 実装**（P1・P2 とも設計は ADR-0117 で済み。App Host へ
   `mount(root, DeliverySink)` する経路を実装）。
3. **P3 スタイル ADR** → sink/IR 拡張。
4. （Todo 系なら）**P4 入力束縛**。
5. ここまでで **`.hybs` 無しのプログラマティックなデモ**が可能。`.hybs` オーサリングは P6。
