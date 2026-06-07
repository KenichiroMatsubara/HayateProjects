# Hayate Glossary

Hayate / Tsubame 周辺で現在使う語彙だけをまとめる。現行仕様の根拠は [`docs/spec.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/spec.md) と各 ADR に置き、ここには古い構想や詳細設計を書き込まない。

## Product Names

**Hayate**:
GPU 描画を担う Rust/WASM 側の UI 基盤。`Element Document Runtime`（element tree・listener dispatch・基本 scroll 等）と `SceneGraph` を保持し、`Scene Renderer` を通じて描画する。
_Avoid_: layout/GPU のみの paint server としてのみ説明する、Hayabusa 中心の説明

**Element Document Runtime**:
`hayate-core` Element Layer 内の軽量 document engine。element tree・listener 登録・bubble/non-bubble dispatch・scroll-view の基本 offset 更新・focus 等を担う。Canvas/HTML 経路の **Document Tree 正本**（ADR-0057）。`:hover` / `:active` / `:focus` を Hayate CSS の一部として保持し、render 時に effective style へ合成する（ADR-0056）。Platform Adapter は raw 入力（pointer / wheel / EditContext 等）をここへ渡す。`hayate-adapter-web` 等は input 変換と描画 flush のみ。host は dispatch 結果を `poll_events()`（または後継 export）で受け取り、listener id に紐づく callback を実行する。慣性 scroll は担わない。
_Avoid_: adapter 層ごとの document semantics、Tsubame 側 bubble、Tsubame 側 shadow tree、Hayate から host への import callback（ADR-0018 参照）

**Tsubame**:
JS/TS 向けのレンダラーターゲット基盤。`Renderer Protocol`・`DOM Renderer`・`Canvas Renderer` を提供し、各フレームワーク固有ランタイムをそのまま持ち込む。
_Avoid_: signal ランタイム、フレームワーク本体

**Hayabusa**:
Hayate 上に構築する Rust フレームワーク構想。ADR-0045 により Hayate とは WIT 境界ではなく Rust crate 依存で接続する。現時点の開発優先は Tsubame 側であり、Hayabusa は長期構想として扱う。
_Avoid_: 現在の最優先実装対象

## Current Contracts

**Hayate Protocol Contract**:
Hayate リポジトリの `proto/spec/` に置く、Hayate-Tsubame 間の機械可読な契約。`opcodes`・`style_tags`・`event_kinds`・`element_kinds`・`unset_kinds`・`modifier_keys` 等を JSON で定義し、`proto/spec/schema/` の JSON Schema で検証する。正本は Hayate 側のみ。Hayate は `proto/generator/` から wire 定数・decode・encode（`codec.rs`）を `proto/generated/` に生成し commit する。Tsubame は npm パッケージ `@hayate/protocol-spec` 経由で Contract を取り込み、`Tsubame/proto/generator/` から wire 定数・TS encode（`codec.ts`）・adapter vocabulary（`catalog.ts` 等）を `Tsubame/proto/generated/` に生成し commit する。`style_tags` の `encodeFrom` は TS 向け入力変換規則（ADR-0055）。Renderer Protocol 独自の surface（`setProperty`・`addEventListener` 購読 API・`resize`）と semantic mutation キュー（`HayateMutationPacket`）は Contract 外として手書きのまま残す。
_Avoid_: YAML 正本、定数だけ生成して mutation/style encode を手書きし続ける運用、Contract から IRenderer 実装まで生成する設計

**apply_mutations**:
Tsubame `Canvas Renderer` が Hayate に渡すフレーム単位バッチ入口。シグネチャは `apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[])`。
_Avoid_: 個別 `element_set_*` 呼び出しを現行 hot path とみなす説明

**Hayate Mutation Packet**:
Tsubame `Canvas Renderer` と Hayate WASM の間で、フレーム内 mutation を `ops/styles/texts` の unified batch として発行する順序契約。HayateMutationPacket は semantic mutation の順序を保持し、flush 時に ADR-0052 の `apply_mutations` 境界へ一括エンコードする。
_Avoid_: DOM Renderer にも要求される汎用 Renderer Protocol、単なる opcode 定数表、slot 配列だけの狭い codec、TypeScript 側の順序管理を永続設計とみなす説明

**ListenerId**:
Hayate Element Document Runtime が `register_listener` で払い出す opaque な listener ハンドル。host（Tsubame Canvas Renderer 等）は `Map<ListenerId, handler>` だけを保持する。bubble path は runtime 側の責務。
_Avoid_: JS 側 element id × event kind ごとの handler map を長期設計とみなす説明

**Event Delivery**:
Hayate が bubble dispatch した結果。`poll_events()` が返す `{ listener_id, event }` エントリ。ADR-0018 export poll モデルの後継。raw platform input event そのものではない。
_Avoid_: raw event を host が decode して bubble する現行 Interaction Stream モデル

**Renderer Protocol**:
Tsubame と各 `Tsubame Adapter` の境界インターフェース。TypeScript では `IRenderer` として表現される。
_Avoid_: Host Interface, signal API

## Rendering Terms

**SceneGraph**:
Hayate が保持する描画用の retained graph。UI フレームワークやコンポーネントツリーそのものではない。
_Avoid_: Virtual DOM, Component Tree

**Scene Renderer**:
`SceneGraph` を消費して描画結果を生成する実装単位。現行の標準候補は Vello、代替候補は `tiny-skia`。実装は `crates/scene-renderers/{vello,tiny-skia}` に置く。`SceneGraph` の walk は `hayate-core` の `render_scene_graph` が一度だけ行い、実装は内部 `ScenePainter` への委譲のみ（ADR-0054）。
_Avoid_: Backend, Driver, adapter 内での SceneGraph walk

**ScenePainter**:
`hayate-core` 内部の walk callback interface。`fill_rect` / `draw_text_run` / `draw_image` と `push_transform` / `pop_transform` / `push_clip_rect` / `pop_clip` を実装する。host 向け公開契約ではない（ADR-0054）。
_Avoid_: Scene Renderer と混同、Platform Adapter の責務

**Render Host**:
surface 初期化、capability 判定、renderer 切替、資源寿命管理を担う外側の協調層。描画そのものは `Scene Renderer` が担う。
_Avoid_: Platform Adapter, Backend

**Renderer Selection Policy**:
どの `Scene Renderer` を優先し、どの条件で採用・不採用にするかを決めるルール。`Render Host` から分離する。
_Avoid_: host に埋め込まれた if 文連鎖

**Renderer Selection Reason**:
`Render Host` が renderer を採用しなかった、または切り替えた理由を表す共通語彙。`WebGpuUnavailable` / `RendererInitFailed` / `SurfaceLost` / `CapabilityUnsupported` / `DisabledByPolicy` など。
_Avoid_: 場当たり的な文字列エラー

## Runtime Boundaries

**Platform Adapter**:
IME、入力、surface、クリップボード、アクセシビリティなどのプラットフォーム依存処理を担う層。Hayate Core はその実装詳細を知らない。
_Avoid_: Renderer, Host

**Tsubame Adapter**:
`tsubame-solid` / `tsubame-vue` / `tsubame-react` の総称。各フレームワーク固有ランタイムを維持したまま、レンダリング先だけを `Renderer Protocol` に向け替える。
_Avoid_: shared component runtime, unified signal runtime

## Historical Terms

**WIT**:
Hayate の公開境界として使っていた過去の設計語彙。ADR-0049 以降、Hayate-Tsubame 間の現行プロトコル正本ではない。現行文書で使う場合は過去設計であることを明示する。

**wit-bindgen**:
WIT ベース設計に付随する歴史用語。現行の Hayate-Tsubame 契約説明では使わない。

## Example Dialogue

> 「今の Hayate と Tsubame の結合点は何？」
> → 「`@hayate/protocol-spec`（`proto/spec/*.json`）と、それに基づく `apply_mutations` / `poll_events` だよ」

> 「Tsubame は signal ランタイムなの？」
> → 「違う。各フレームワークの既存ランタイムをそのまま使い、レンダリング先だけを差し替える基盤だよ」

> 「Hayabusa は消えたの？」
> → 「消えていない。Rust 側の長期構想として残っている。ただし現状の開発優先は Tsubame 側」
