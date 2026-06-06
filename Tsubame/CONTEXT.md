# Tsubame Glossary

Tsubame の現行語彙だけをまとめる。詳細仕様は [`docs/tsubame-spec.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Tsubame/docs/tsubame-spec.md) と各 ADR を参照する。

## Core Terms

**Tsubame**:
JS/TS 向けのレンダラーターゲット基盤。`Renderer Protocol`・`DOM Renderer`・`Canvas Renderer` を提供する。フレームワーク本体や signal ランタイムではない。
_Avoid_: unified signal runtime

**Renderer Protocol**:
Tsubame と各 adapter の境界インターフェース。TypeScript では `IRenderer` として表現する。
_Avoid_: Host Interface

**Canvas Renderer**:
Hayate 向け renderer 実装。JS 側でフレーム分の変更を積み、`apply_mutations(ops: Float64Array, styles: Float32Array)` を 1回/frame 呼ぶ。
_Avoid_: 個別 `element_set_*` 呼び出しを現行 hot path とみなす説明

**DOM Renderer**:
ブラウザ DOM へ直接反映する renderer 実装。Renderer Protocol のもう一つの実装。Z-Order は Hayate と同じ RN 方式（兄弟内のみ・後勝ち・root 配置で親をまたぐ）を、RN Web 現行方式（全 Element kind に `position: relative` + デフォルト `zIndex: 0`）で DOM 上にエミュレートする。ブラウザ CSS との完全一致は目標にしない。Canvas / Hayate と Z-Order が乖離しうるスタイルを設定した場合は開発時に警告する（拒否はしない）。警告対象は拡張可能な registry で管理し、現行は `opacity`、Element スタイルに追加された `transform` を含む。静的コードへの警告は `DomStylePatch` の TypeScript `@deprecated` で IDE ファイル診断とする（ビルド失敗なし、`tsubame lint` CLI は設けない）。動的値は dev のみ runtime `console.warn`（同一 element + property につきセッション 1 回）。将来 ESLint によるファイル警告は理想だが初期スコープ外。
_Avoid_: SSR, hydration, Hayate HTML Mode（Hayate 不使用のため）

## Integration Terms

**protocol.yaml**:
Hayate-Tsubame 間プロトコル定数の単一正本。`apply_mutations` / `poll_events` の enum、opcode、style tag はここに従う。

**apply_mutations**:
Canvas Renderer のフレームバッチ入口。Tsubame と Hayate の結合点の中心。

**Interaction Stream**:
Canvas Renderer 内で `poll_events()` 由来の raw events を decode → filter → bubble → handler dispatch する Module。`createInteractionStream(options)` で生成し、`dispatchRawEvents` / `dispatchParsedEvent` / `dispatchOne` の3層 Interface を持つ。bubbling policy（click / input / keydown はバブル、focus / blur / hover-enter / hover-leave は非バブル）はこの Module に集約する。無視ポリシー（composition / scroll / resize / active / pointer_move / fetch_font は現状 no-op）も table として明示する。
_Avoid_: event bus, event emitter（状態を持たない dispatch Module である）

**Tsubame Adapter**:
`tsubame-solid` / `tsubame-vue` / `tsubame-react` の総称。各フレームワーク固有ランタイムを維持しつつレンダリング先だけを差し替える。
_Avoid_: shared component runtime

## Related Products

**Hayate**:
Tsubame が Canvas Renderer 経由で利用する描画基盤。Tsubame は Hayate の内部実装には依存せず、`protocol.yaml` と `apply_mutations` / `poll_events` 契約だけを見る。

**Hayabusa**:
Rust 側の長期構想。Tsubame は Hayabusa の JS 版ではない。

## Example Dialogue

> 「Tsubame は framework？」
> → 「違う。framework 固有ランタイムをそのまま使い、描画先を差し替える基盤」

> 「Hayate との結合点は？」
> → 「`protocol.yaml` と `apply_mutations` / `poll_events`」
