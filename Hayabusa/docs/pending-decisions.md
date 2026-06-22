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

---

## P1 🔴 レンダリング統合とアプリ起動

**問題**：ADR-0006 は「Hayabusa は hayate-core に path 依存し ElementTree を駆動する」までしか
決めていない。デモを画面に出すには、以下が未決。

- **アダプタ方針**：既存の `hayate-adapter-web`（GPU/DOM 描画＋ポインタ/キー入力＋IME。
  現状は proto/wire の `apply_mutations` バッチ経由で Tsubame が駆動する）を **再利用するのか**、
  Hayabusa 専用アダプタを持つのか。
- **ElementTree の所有位置**：Hayabusa（Rust）が ElementTree を直接所有して駆動するのか、
  それとも wire 境界（`apply_mutations`）越しに既存アダプタの ElementTree を駆動するのか。
- **boot / フレームループ**：mount API（例：`mount(component, root, adapter)`）と、
  `render(timestamp_ms)` フレームループ（transition・カーソル点滅）の所有者。
- **ビルド現実（実装ブロッカー）**：hayate-core は vendored crate を `[patch.crates-io]`
  （Hayate ワークスペース）で差し替えており、別ワークスペースの Hayabusa から path 依存で
  リンクすると patch が効かない可能性。wasm 同梱・パッケージングも未検証。

**現在の代替**：`ElementSink`（`src/sink.rs`）が ElementTree の API に 1:1 で写るシームに
なっており、テストは `RecordingSink` で fine-grained patch を観測している。実 ElementTree を
駆動する `HayateSink` はこの ADR の決定待ち。

**ADR にすべきこと**：「Hayabusa レンダリング統合とアプリ起動」（アダプタ再利用 vs 専用・
ElementTree 所有位置・boot/フレームループ・ビルド/パッケージング）。

**推奨**：ADR 確定の前に **spike**（hayate-core を path 依存で Hayabusa から実際にビルドできるか
＝クロスワークスペース問題の検証）を先に行う。

## P2 🔴 イベント入力の経路

**問題**：現在の `Instance::click(ElId)`（`src/instantiate.rs`）はテスト用シーム。実機では
Hayate の Interaction Event／hit-test／`poll_deliveries`（Hayate ADR-0053）／`register_listener`
を経由する。テンプレの `on:click` / `on:input` 等のハンドラを **Hayate のイベント配送へ
どう束ね、要素単位に dispatch し、flush 合体（ADR-0003）へ載せるか**の決定が無い。

**ADR にすべきこと**：「テンプレートのイベントハンドラ ↔ Hayate イベント配送」。P1 と一本に
まとめても良い。

## P3 🟠 スタイル（`<style>` DSL → Hayate CSS）

**問題**：CONTEXT.md は「`<style>` は言語非依存の DSL」「Hayate CSS は要素ローカルの
インラインスタイル」と定義するだけ。未決は次の通り。

- オーサリング面（プロパティ集合・単位・`:hover`/`:active`/`:focus` の素通し）。
- **静的 vs reactive なスタイル束縛**（条件付きクラス相当・`{expr}` 駆動の style）。
- スコープ（コンポーネント単位の scoped style を持つか）。

加えて、**sink / Template IR に現状 style オペが無い**（`set_style` 等の拡張が要る）。
これが無いとレイアウト・色が出ず「見せられるデモ」にならない。

**ADR にすべきこと**：「Hayabusa スタイル DSL と束縛」＋ sink/IR 拡張。

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

1. **P1 の spike**（クロスワークスペース・ビルド可否の検証）
2. **P1＋P2 を一本の ADR に**（レンダリング統合・イベント入力）→ `HayateSink` 実装
3. **P3 スタイル ADR** → sink/IR 拡張
4. （Todo 系なら）**P4 入力束縛**
5. ここまでで **`.hybs` 無しのプログラマティックなデモ**が可能。`.hybs` オーサリングは P6。
