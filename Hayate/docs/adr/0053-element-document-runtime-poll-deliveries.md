# Element Document Runtime を hayate-core に置き、poll deliveries で host に通知する

**Status: accepted**

**Date: 2026-06-06**

## Context

Hayate-Tsubame 結合点の整理（Hayate Protocol Contract の JSON 化、codegen 統合）を進める中で、モグラたたきの根が **wire 定数のドリフト** ではなく **document runtime の責務漏れ** であることが明らかになった。

現状 Canvas 経路では:

- Hayate `ElementTree` が parent/child と scroll offset を保持している
- 一方 Tsubame `CanvasRenderer` が第二の parent map を維持し、`InteractionStream` が JS 側で bubble している
- Canvas Mode の `on_wheel` は既に Platform Adapter 内で scroll offset を積算しているが、ADR-0022 文書と矛盾していた
- `fetch_font` は Hayate 内部で消費され、IME とは無関係

Hayate の目標像は **Hayate tag + Hayate CSS を渡せば描画できる軽量 DOM** である。Framework（Tsubame Adapter）の reactivity は残しつつ、listener / bubble / 基本 scroll など DOM engine 側の責務を JS で無理やり拾う設計は、パフォーマンス理由ではなく Renderer Protocol 共通化の副作用として漏れた。

## Decision

### 1. Element Document Runtime（hayate-core）

`hayate-core` Element Layer に **Element Document Runtime** を置く。

**Runtime が担う:**

- element tree 上の listener 登録
- bubble / non-bubble dispatch
- focus 等の document セマンティクス
- scroll-view の **基本 offset 更新**（wheel delta の clamp 付き積算）— セマンティクスは core、raw 入力は Platform Adapter 経由

**Runtime が担う（ADR-0056 で追加）:**

- `:hover` / `:active` / `:focus` 擬似状態スタイルの保持と render 時 effective style 解決

**Runtime が担わない:**

- 慣性・rubber-band・snap 等の scroll 物理（Platform Adapter 責務、ADR-0046 と整合）
- ElementId の JS 採番（ADR-0005 — batch 最適化。温存）

`hayate-adapter-web` 等の Platform Adapter は **raw 入力の変換** と **描画 flush** のみ。Canvas Mode / HTML Mode は同一 runtime のセマンティクスを共有する。

Tsubame DOM Renderer はブラウザ native document を使う別経路であり、本 ADR の対象外。

### 2. Poll deliveries（ADR-0018 の進化）

Hayate は host へ **import callback しない**（ADR-0018 維持）。

- `register_listener(element_id, event_kind) -> ListenerId` を export
- runtime が bubble dispatch 後、キューに **Event Delivery** `{ listener_id, event }` を積む
- host が `poll_events()` で drain し、`ListenerId` に紐づく handler を実行

Tsubame `InteractionStream` の raw event decode + JS bubble + `IGNORED_KINDS` は廃止方向。host は `Map<ListenerId, Handler>` の thin map のみ残す。

`wireRole` による event 分類:

| wireRole | 例 | host への届け方 |
|----------|-----|----------------|
| `interaction` | click, scroll（通知）, hover_* | delivery（`adapterTier: forward` / `deferred`） |
| `ime` | composition_* | delivery（初期は `deferred`） |
| `hayate-internal` | fetch_font | 届けない（runtime / adapter 内で完結） |
| `host-echo` | resize（wire） | 届けない。viewport は adapter（web の `hayate-adapter-web`）/ native（android ループ）が `set_viewport` を直接駆動する（ADR-0080 を native へ延長, #475）。`resize` は Renderer Protocol surface ではない |

### 3. Hayate Protocol Contract（JSON + 分割 spec）

Hayate-Tsubame 間契約の正本は **Hayate リポジトリ** `proto/spec/` の JSON 群とする。

- 8 セクション分割: `opcodes`, `style_tags`, `event_kinds`, `element_kinds`, `unset_kinds`, `modifier_keys`, `types`, `enums`
- `proto/spec/schema/` で JSON Schema 検証
- 擬似 YAML および手書き line parser は廃止

配布: npm パッケージ `@torimi/hayate-protocol-spec`。Tsubame は正本を持たず依存として取り込む。HayateProjects モノレポは AI/クラウド作業都合であり、アーキテクチャ上の結合点ではない。

### 4. Generator / generated の配置

| リポジトリ | 所有 |
|-----------|------|
| Hayate | `proto/spec/`（正本）、`proto/generator/`（Rust）、`proto/generated/`（Rust 成果物） |
| Tsubame | `proto/generator/`（TS）、`proto/generated/`（TS 成果物） |

両 repo の `proto/generated/` は commit し、CI で `generate` → diff check する。

Tsubame 生成射程は **wire + adapter vocabulary**（`StylePatch`, `EventKind`, semantic mutation surface, delivery wire 型）。`IRenderer` の tree/style/imperative メソッド型は generated vocabulary の thin wrapper。`setProperty` / `addEventListener` 購読 API は Renderer Protocol 独自 surface として Contract 外。`resize` も Contract 外である点は同じだが**もはや Renderer Protocol surface ではない** — viewport 追従は host→adapter→core（web は `hayate-adapter-web` の自己配線 ResizeObserver、android は native ループ）が `set_viewport` で所有し、`IRenderer.resize` / `RawHayate.on_resize` は撤去済みで Tsubame は resize 経路から外れる（ADR-0080 を native へ延長, #475。当初の「resize は Renderer Protocol surface」記述の訂正）。

### 5. Scroll と ADR-0046 の関係

- **基本 wheel offset 更新のセマンティクス**（nearest scroll-view 探索、clamp、`element_set_scroll_offset`）は hayate-core Document Runtime に集約する
- **Platform Adapter** は raw wheel/touch を受け、runtime の API を呼ぶ。慣性・snap 等は adapter 責務（ADR-0046 維持）
- `scroll` delivery は parallax / lazy load 等の **アプリ通知専用**。offset 積算目的には使わない（ADR-0046 維持）
- `element_set_scroll_offset` は **プログラマティックスクロール専用** API として残す（ADR-0046 維持）

## Considered Options

- **現状維持（Tsubame InteractionStream + JS bubble）**: wire/codegen 整備だけでは JS document shim が残り、モグラが止まらないため却下
- **WASM Closure callback（Hayate から直接 JS invoke）**: レイテンシは良いが ADR-0018 の export-only 原則と緊張するため却下
- **scroll を upper layer（Hayabusa）管理（ADR-0022）**: Canvas 実装と逆行。ADR-0046 で既に却下済み
- **Document Runtime を hayate-adapter-web のみに置く**: Canvas/HTML 二重実装が残るため却下

## Consequences

- `hayate-core` に document runtime module を新設。listener registry + dispatch + scroll 基本 API
- `poll_events` wire 形式を Event Delivery 向けに拡張（Hayate Protocol Contract JSON spec で定義）
- Tsubame `CanvasRenderer` から `parentOf`（bubble 用）、`InteractionStream` を削除
- `Hayate/proto/protocol.yaml` を JSON spec 8 ファイルへ移行。`build.rs` 手書き YAML parser 削除
- `@torimi/hayate-protocol-spec` npm パッケージ新設
- ADR-0049 の正本形式を YAML から JSON spec ディレクトリへ更新（spirit 維持、形式変更）
- ADR-0022 は ADR-0046 により既に superseded。本 ADR は scroll 基本セマンティクスの core 集約を追記

## Supersedes / Amends

- **Amends ADR-0049** — 正本形式を `protocol.yaml` 単体から `proto/spec/*.json` + JSON Schema へ
- **Amends ADR-0018** — raw event poll から delivery poll へ進化（export poll 原則は維持）
- **Amends ADR-0046** — scroll 基本セマンティクスを hayate-core runtime に集約。物理演算は Platform Adapter のまま
- **Extended by ADR-0057** — `parentOf` 撤去に加え、`tsubame-solid` shadow tree 撤去で document tree 正本を backend 一箇所に限定
