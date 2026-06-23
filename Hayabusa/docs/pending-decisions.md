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
| クロスワークスペース・リンク（実 hayate-core 駆動の `HayateSink`） | 0009 | ✅ spike 実証・実装済 |
| App Host 配線（`DeliverySink`・borrowed-tree・Click ルーティング） | 0117 / 0009 | ✅ 実装済（`tests/app_host.rs`） |
| レンダリング統合・boot・フレームループ（描画 present） | 0117 | ✅ 方針決定済（Platform 統合は未） |

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

**spike 解消（2026-06-23・ADR-0009）**：クロスワークスペースのビルド spike を実施し、
**`[patch.crates-io]` を Hayabusa 側に複製すれば hayate-core を path 依存でリンクできる**ことを
実証した。bare path 依存（patch なし）は crates.io の vendored でない `fontique` を解決して失敗、
patch 複製で成功。`HayateSink`（`src/hayate_sink.rs`）を `feature = "hayate-core"` で実装し、
counter tracer bullet を実 `ElementTree` 上で駆動する統合テスト（`tests/hayate_sink.rs`）が緑。
詳細は [ADR-0009](adr/0009-cross-workspace-link-to-hayate-core-via-patch-replication.md)。

**App Host 配線も実装済み（2026-06-23・ADR-0117）**：`HayabusaApp`（`src/app_host.rs`）を
共有 `hayate_app_host::AppHost` へ `DeliverySink` として mount する経路を実装した。App Host が
tree を所有する borrowed-tree モデルの生存期間ギャップは **buffering（effect が `RecordingSink`
へ `Mutation` を積み、`handle` がフレーム内で借用ツリーへ drain）** で解き、unsafe を使わない。
click は mount 時に登録した `ListenerId → ElId → handler` でルーティングする。`tick →
poll_deliveries → handle → flush → 借用ツリーへ patch` の 1 フレーム完全ループを
`tests/app_host.rs` で実証（`feature = "app-host"`）。

**残る実装タスク（ブロッカーではない）**：Click 以外のイベント（`on:input` 等は P4・ADR-0007 の
経路）、描画 present を伴う Platform Front / Adapter 統合、wasm 同梱・パッケージングは未検証。

## P2 🟢 イベント入力の経路 — **ADR-0117 で決着**

**決着**：App Host が `tick` フェーズ1で `poll_deliveries()` を drain し、フェーズ2で mount 時登録の
`DeliverySink` へ drain 済み `{listener_id, event}` バッチを同期 push する。Hayabusa の DeliverySink は
自身が所有する `ListenerId → handler` map を引いて handler を実行し、reactive graph を flush して
（handler 由来・非同期由来とも flush 点はこの 1 箇所）、in-process で `&mut ElementTree` に mutation を
出し切ってから return する。App Host は `ListenerId` の意味も handler の存在も知らない（consumer 非依存）。
テンプレの `on:click` / `on:input` は handler を ListenerId に紐付けて map に登録するだけ。

**実装済み（2026-06-23・ADR-0117）**：`HayabusaApp`（`src/app_host.rs`）の `DeliverySink::handle`
が `poll_deliveries` 済みの `{listener_id, event}` を `ListenerId → ElId → Instance::click` で
ルーティングし、handler 実行＋flush 後に借用ツリーへ mutation を drain する。`Instance::click` は
テスト用シームとして残しつつ、実機経路は delivery 駆動になった。新しい ADR は不要だった。
（Click のみ。`on:input` 等は P4・ADR-0007 の経路で後続）。

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

## P4 🟢 フォーム／`text-input` の双方向束縛 — **ADR-0007 で決着**

**問題**：controlled input の `value` 束縛、`on:input` / `on:submit`、EditIntent 統合
（Hayate 側は EditState/EditIntent を所有）。Todo 系デモに必須。

> **決定（2026-06-23）：二重正本を作らない。編集中の単一正本は Hayate core の `EditState`。**
> Hayate core が `EditState`（`text_content` / `preedit`〔IME 組成中〕/ `cursor_byte_index` ＋
> insert/backspace/commit、ADR-0069）と編集セマンティクスを既に所有しているため、Hayabusa の
> `value` signal はこれと並ぶ正本にしない。controlled の体験は次の非対称で実現する：
>
> - **読み（主）**：`on:input` が commit 済み `text_content` を delivery で運び、Hayabusa が signal を
>   更新する（DeliverySink 経路・P2 と同じ）。
> - **書き（従・programmatic set のみ）**：`value={signal}` は programmatic set に限る（例：submit 後に
>   signal を空にしフォームをクリア）。Hayate が EditState と突き合わせ、**差分があり、かつ組成中で
>   ないときだけ**適用する。毎キーストロークでは書き戻さない。
> - これにより IME 組成・カーソル・選択は EditState が一手に握り、controlled input + IME の二重正本
>   破綻を回避する。「完全 controlled（毎入力で要素へ書き戻す）」は採らない。
>
> **ADR 化済み**：[ADR-0007](adr/0007-input-value-binding-editstate-single-source-of-truth.md)
> （「`text-input` の value 束縛：編集中の単一正本は Hayate `EditState`、signal はミラー」）。
> sink/IR には programmatic な value set オペ（差分・非組成中ガード付き）が要る（実装タスク）。

## P5 🟡 Store（コンポーネント横断状態）

**問題**：CONTEXT.md に Store 語彙はあるが ADR 無し。非自明なデモは共有状態を欲しがる。
ADR-0003 の Scope 階層に乗る provide/inject で小さく実装できる見込み。Router は語彙のみで
ADR 無し（初回デモには通常不要）。

> **段階づけ（2026-06-23）**：初回 Todo デモは **単一コンポーネント（Store 不要）** で組む。
> ただしこれは**あくまで第一段階**であり、**第二段階で複数コンポーネント分割＋ Store による
> 共有状態へ進む**ことを前提とする。Store はその第二段階に入る時点で本項を独立 ADR 化する
> （provide/inject の Scope への載せ方を決める）。単一コンポーネントを終着点にはしない。

**ADR にすべきこと**：「Store の provide/inject と Scope への載せ方」（第二段階で起こす）。

## P6 🟢 `.hybs` ファイル＋`<script>` コンパイル — **第一段階で必須・ADR-0008 で決着**

**問題**：`<template>` / `<style>` マークアップのパーサは ADR-0004＋CONTEXT から導出可
（低リスク）。難所は **Rust-native `<script>` のコンパイル/登録経路**（ADR-0001 は「境界ゼロ」
と言うが具体 ADR が無かった）。

> **決定（2026-06-23）：初回デモは `.hybs` をコンパイルした出力として動かす（手組みにしない）。**
> `.hybs` であること自体に意味があり、第一段階からオーサリング面を見せる。コンパイル機構は
> **build 時 codegen**（build.rs / 専用 codegen バイナリが `.hybs` をパースし生成 Rust を出力）。
> `<template>`→Template IR 構築コード、`<style>`→static style セット、Rust `<script>`→setup 関数
> としてそのまま差し込み（cargo が型検査）。proc-macro インラインや手組みは却下。
> [ADR-0008](adr/0008-hybs-compiles-to-generated-rust-via-build-codegen.md)。
>
> 他言語 script（wasm ゲスト・ADR-0001/0002）は射程外で後続。

---

## デモ到達への最短経路（メモ）

初回デモは **Todo 系を `.hybs` コンパイル出力として**動かす（単一コンポーネント＝第一段階・P5）。

1. ~~**P1 の spike**（クロスワークスペース・ビルド可否の検証）~~ ✅ **完了（ADR-0009）**：
   patch 複製でリンク可。`HayateSink` で実 `ElementTree` を駆動できることを実証済み。
2. ~~**App Host への配線**（P1・P2 とも設計は ADR-0117 で済み）~~ ✅ **完了（ADR-0117）**：
   `HayabusaApp` を `DeliverySink` として App Host へ `mount` し、buffering で借用ツリーモデルへ
   載せ、Click を ListenerId → ElId → handler でルーティング（`tests/app_host.rs`）。
3. **P3 static style** → sink/IR の `set_style` 拡張。
4. **P4 入力束縛**（ADR-0007・EditState 単一正本）→ sink/IR の programmatic value set オペ。
5. **P6 `.hybs` codegen**（ADR-0008・build 時 codegen）→ `<template>`/`<style>` パーサ＋
   Rust `<script>` 差し込み。これで Todo の `.hybs` がコンパイルされ画面に出る。
6. 第二段階：複数コンポーネント分割＋ **P5 Store**（要 ADR）。
