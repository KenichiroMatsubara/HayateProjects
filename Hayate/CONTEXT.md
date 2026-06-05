# Hayate Glossary

Hayate / Tsubame 周辺で現在使う語彙だけをまとめる。現行仕様の根拠は [`docs/spec.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/spec.md) と各 ADR に置き、ここには古い構想や詳細設計を書き込まない。

## Product Names

**Hayate**:
GPU 描画を担う Rust/WASM 側の描画基盤。`SceneGraph` を保持し、`Scene Renderer` を通じて描画する。現行の外部契約は Hayabusa 向けの Rust 直結と、Tsubame 向けの `protocol.yaml` ベース契約が中心。
_Avoid_: WIT が現行の単一正本であるという説明、Hayabusa 中心の説明

**Tsubame**:
JS/TS 向けのレンダラーターゲット基盤。`Renderer Protocol`・`DOM Renderer`・`Canvas Renderer` を提供し、各フレームワーク固有ランタイムをそのまま持ち込む。
_Avoid_: signal ランタイム、フレームワーク本体

**Hayabusa**:
Hayate 上に構築する Rust フレームワーク構想。ADR-0045 により Hayate とは WIT 境界ではなく Rust crate 依存で接続する。現時点の開発優先は Tsubame 側であり、Hayabusa は長期構想として扱う。
_Avoid_: 現在の最優先実装対象

## Current Contracts

**protocol.yaml**:
[`proto/protocol.yaml`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/proto/protocol.yaml) は Hayate-Tsubame 間プロトコル定数の単一正本。`opcodes`・`style_tags`・`event_kinds`・`element_kinds`・`unset_kinds`・`modifier_keys` を定義する。
_Avoid_: WIT 由来の定数定義、TS/Rust 手書きの定数表

**apply_mutations**:
Tsubame `Canvas Renderer` が Hayate に渡すフレーム単位バッチ入口。シグネチャは `apply_mutations(ops: Float64Array, styles: Float32Array)`。
_Avoid_: 個別 `element_set_*` 呼び出しを現行 hot path とみなす説明

**Hayate Mutation Packet**:
Tsubame `Canvas Renderer` と Hayate WASM の間で、typed batch mutations（`ops/styles`）と batch 外の低頻度 string/unset 呼び出しの発行順序を規定する、CanvasRenderer–Hayate WASM 間の Hayate 固有の契約。現行は WASM 境界の型制約により TypeScript 側（HayateMutationPacket）が発行順序を管理しているが、これは一時的な補償設計である（ADR-0052 参照）。
_Avoid_: DOM Renderer にも要求される汎用 Renderer Protocol、単なる opcode 定数表、slot 配列だけの狭い codec、TypeScript 側の順序管理を永続設計とみなす説明

**poll_events**:
Hayate から Tsubame 側へ返すイベント列。イベント種別とフィールド名は `protocol.yaml` を正本とする。
_Avoid_: event kind の手書き switch を正本とみなす説明

**Renderer Protocol**:
Tsubame と各 `Tsubame Adapter` の境界インターフェース。TypeScript では `IRenderer` として表現される。
_Avoid_: Host Interface, signal API

## Rendering Terms

**SceneGraph**:
Hayate が保持する描画用の retained graph。UI フレームワークやコンポーネントツリーそのものではない。
_Avoid_: Virtual DOM, Component Tree

**Scene Renderer**:
`SceneGraph` を消費して描画結果を生成する実装単位。現行の標準候補は Vello、代替候補は `tiny-skia`。
_Avoid_: Backend, Driver

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
> → 「`protocol.yaml` と、それに基づく `apply_mutations` / `poll_events` だよ」

> 「Tsubame は signal ランタイムなの？」
> → 「違う。各フレームワークの既存ランタイムをそのまま使い、レンダリング先だけを差し替える基盤だよ」

> 「Hayabusa は消えたの？」
> → 「消えていない。Rust 側の長期構想として残っている。ただし現状の開発優先は Tsubame 側」
