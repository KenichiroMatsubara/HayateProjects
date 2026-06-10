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

**Hayate Protocol Contract**:
Hayate リポジトリ `proto/spec/` の JSON 契約定義群（JSON Schema で検証）。Tsubame は npm パッケージ `@hayate/protocol-spec` 経由で取り込み、`Tsubame/proto/generator/` から wire 定数と adapter vocabulary（`StylePatch`・`EventKind`・semantic mutation surface 等）を `Tsubame/proto/generated/` に生成し commit する。`setProperty`・`addEventListener` 購読 API・`resize` は Renderer Protocol 独自 surface として Contract 外。
_Avoid_: wire only 生成、adapter 向け型の手書き維持、Contract から Renderer 実装まで生成する設計

**apply_mutations**:
Canvas Renderer のフレームバッチ入口。Tsubame と Hayate の結合点の中心。

**Interaction Stream**:
（移行対象）Canvas Renderer 内の JS 側 event dispatch Module。`Element Document Runtime` 移管後は Hayate 内 dispatch に置き換え、Tsubame 側は host callback のみ残す。
_Avoid_: 長期設計として Tsubame 側 bubble を正とする説明

**Tsubame Adapter**:
`tsubame-solid` / `tsubame-vue` / `tsubame-react` の総称。各フレームワーク固有ランタイムを維持しつつレンダリング先だけを差し替える。**描画正本**は持たず、`ElementId` ハンドルと mutation を `IRenderer` へ届ける。例外として `tsubame-solid` は `solid-js/universal` の同期走査要件のため構造専用 shadow tree（reconcile index）を保持する（ADR-0062 が ADR-0057 を supersede。§Shadow Tree 参照）。
_Avoid_: shared component runtime, shadow document tree

**Shadow Tree（構造専用 reconcile index）**:
`tsubame-solid` が `solid-js/universal` のツリー走査 API（`getParentNode` / `getFirstChild` / `getNextSibling`）を同期で満たすために JS 側に保持する `TsubameNode` 構造（`parent` / 順序付き `children` / `elementKind`）。`solid-js/universal` は VDOM を持たず reconcile 時にホスト構造を同期で読むため、正本ツリーが WASM batch 境界の向こう（Hayate）にある Canvas 経路では近側に構造インデックスが不可避。**正式採用**（ADR-0062 が ADR-0057 の撤去方針を覆す）。diff されないため VDOM ではない。描画正本（text 内容・style・layout）は backend が持ち、shadow は構造のみ。CPU は signal 経路で +0、メモリ増分 ~70 B/node。
_Avoid_: VDOM（diff しないので該当しない）、描画正本の複製、`text` 内容を shadow の正本とする設計、tsubame-react / tsubame-vue にも shadow を要求する説明（VDOM reconciler は不要）

**Text Element**:
Solid の文字列・`createTextNode` の正本表現。Hayate `ElementKind::Text` として Canonical Tree の子に載せる。`button` 直下のラベルも子 `text` element とする（ADR-0058）。性能が拮抗し計測で優劣がつかない場合、DOM Renderer の構造（`button` > `span`）を仕様 tie-break とする。
_Avoid_: 仮想 TextNode、親への `setText` 集約、Hayate 未登録の負 ID

## Related Products

**Hayate**:
Tsubame が Canvas Renderer 経由で利用する描画基盤。Tsubame は Hayate の内部実装には依存せず、`@hayate/protocol-spec`（`proto/spec/*.json`）と `apply_mutations` / `poll_events` 契約だけを見る。

**Hayabusa**:
Rust 側の長期構想。Tsubame は Hayabusa の JS 版ではない。

## Example Dialogue

> 「Tsubame は framework？」
> → 「違う。framework 固有ランタイムをそのまま使い、描画先を差し替える基盤」

> 「Hayate との結合点は？」
> → 「`@hayate/protocol-spec` と `apply_mutations` / `poll_events`」
