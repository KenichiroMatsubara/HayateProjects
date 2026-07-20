# Tsubame Glossary

Tsubame の現行語彙だけをまとめる。詳細仕様は [`docs/tsubame-spec.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Tsubame/docs/tsubame-spec.md) と各 ADR を参照する。

## Core Terms

**Tsubame**:
JS/TS 向けのレンダラーターゲット基盤。`Renderer Protocol`・`DOM Renderer`・`Hayate Renderer` を提供する。フレームワーク本体や signal ランタイムではない。
_Avoid_: unified signal runtime

**Renderer Protocol**:
Tsubame と各 adapter の境界インターフェース（app↔renderer）。TypeScript では `IRenderer` として表現する。host↔renderer の境界ではない点に注意（→ `Host`）。
_Avoid_: Host Interface

**Host**:
renderer に platform への結合点を与える側。Tsubame は host を *掴みに行かず*、注入されたハンドルとして *受け取る*（**Host 結合原則**）。
- **Canvas 経路**: host は描画先と分離する（Hayate が surface を描く）。renderer は frame-clock の tick だけを受け取り、surface・resize・pointer・IME は host が構築した adapter（`hayate-adapter-web` / native）が所有する（ADR-0080 を native まで延長）。renderer は **platform 識別子をゼロ保持**（`HTMLCanvasElement` 型も canvas 参照も持たない）。「Tsubame は host を知らない」の強形はここに全適用。
- **DOM 経路**: host == 描画先（DOM）。`container` / `doc` を注入で受け取る。可搬性を主張しない公言された browser 結合経路なので DOM 結合でよい。弱形「受け取る、掴みに行かない」。
_Avoid_: renderer が canvas / global / DOM を掴みに行く設計、`canvas: null` で host 知識を無効化して native を成立させる構造（知識が型に残るため原則破り）

**Composition Root（合成ルート / `runTsubameApp`）**:
target 選択・`Host` 配線・renderer 取得・mount を一つの interface の裏へ畳む App 階層の deep module。`@torimi/tsubame-app` に置き、`@torimi/tsubame-renderer-protocol` だけに依存する（`renderer-dom` / `renderer-hayate` も `@torimi/hayate-host` も import しない＝Hayate ランタイム盲目）。`runTsubameApp(host: Host, mount: TsubameMount): Dispose` を公開し、`Host.createRenderer(): IRenderer | Promise<IRenderer>`（＋optional `stop()`）で得た renderer を `mount` に渡すだけ。具体 renderer 名（`DomRenderer` / `HayateRenderer`）も platform も知らない — それらは `Host` 実装に局在する。web 専用 helper `shouldUseDomRenderer` は明示 `dom` と EditContext 欠如による DOM 退避だけを判定し、Canvas backend の語彙・順序・query 解釈は `@torimi/hayate-host` が所有する（ADR-0012）。
_Avoid_: orchestrator が `renderer-dom` / `renderer-hayate` / `@torimi/hayate-host` を import する設計、FW ごとに別合成ルートを書く設計、Canvas backend 名・選択順・検出結果を Tsubame App に持たせる設計

**Tsubame Mount（`TsubameMount`）**:
合成ルートにおける唯一の FW 固有 seam。`(renderer: IRenderer) => Dispose` 型で、各 `Tsubame Adapter`（solid / react / vue）が自分の reactivity でツリーを `IRenderer` に mount する 1 関数として供給する。solid は `() => JSX`、react は `ReactNode` という `renderTsubame` の呼び形の差を内側に閉じ込め、合成ルートには一様な形で現れる。FW を増やすコストはこの 1 関数に縮む（platform 増殖は `Host`、renderer は Dom/Hayate の二つで固定・ADR-0012）。
_Avoid_: mount の呼び形差を orchestrator に漏らす設計、FW ごとに host 配線・mode 検出を再実装する設計、`renderTsubame` の thunk/element 差を不当な非対称とみなし統一しようとする設計

**Host bootstrap**:
surface 取得・Hayate ランタイム構築（WASM ロード / WebGPU プローブ / backend 選択 / native RawHayate 注入）・clock 源の確立を行う配線。**Tsubame の renderer パッケージには属さない** — Hayate ランタイム側（web adapter / native）または App（合成ルート）が持つ。具体的には Hayate 側 JS パッケージ `@torimi/hayate-host`（`createHayateWebHost(canvas) → {raw, requestFrame, cancelFrame}` と `./native` の `createHayateNativeHost(raw)`）が web/native の host を供給する（#477）。App は host から `RawHayate`（+ clock）を受け、`new HayateRenderer({ raw, requestFrame, cancelFrame })` して mount する（合成ルート helper は `examples/solid-demo` の `mountCanvasApp`、`@torimi/tsubame-app` の `runTsubameApp` へ昇格予定・ADR-0012）。browser/native はこの形で対称（docs/adr/0004）。
_Avoid_: `@tsubame/renderer-canvas` 内に `init.ts` / `init-android.ts` 等の host bootstrap を置く設計、Tsubame が `hayate-adapter-web` に依存する設計、WASM 巻き込み回避のための `/android` サブパス分離

**Hayate Renderer**:
Hayate 向け renderer 実装。JS 側でフレーム分の変更を積み、`apply_mutations(ops: Float64Array, styles: Float32Array)` を 1回/frame 呼ぶ。host を知らない：受け取るのは frame-clock の tick だけで、canvas・resize・pointer・IME は host 側 adapter が所有する（Host 結合原則の強形）。DOM Renderer と対をなし、両者はターゲット（Hayate / DOM）で区別する。**プラットフォーム盲目**で、Hayate が走る全プラットフォーム（web-WASM / Android native / 将来 iOS・Desktop）で不変 — platform 差は `raw` を供給する `Host` 側に宿る（`Hayate Renderer` を入れること自体は Android 対応を意味しない。native `Host` が Android 対応を成立させる）。旧称 **Canvas Renderer** は HTML `<canvas>` との混同を避けターゲット名へ改名した（ADR-0011）。名にあった "Canvas" は HTML `<canvas>` 要素ではなく **Hayate が描く即時描画サーフェス**（"Hayate canvas" / h-canvas; Android `Canvas`・Skia・Flutter `Canvas` と同義）を指していた。HTML `<canvas>` は browser host が持つ surface ハンドルにすぎず、Android では native Surface に置き換わる。
_Avoid_: Canvas Renderer（旧称・ADR-0011）, 個別 `element_set_*` 呼び出しを現行 hot path とみなす説明、`HayateRenderer` が `HTMLCanvasElement` を型に持つ設計、`resize()` を renderer に置く設計、`Hayate Renderer` 採用＝自動 Android 対応とみなす理解

**DOM Renderer**:
ブラウザ DOM へ直接反映する renderer 実装。Renderer Protocol のもう一つの実装。Z-Order は Hayate と同じ RN 方式（兄弟内のみ・後勝ち・root 配置で親をまたぐ）を、RN Web 現行方式（全 Element kind に `position: relative` + デフォルト `zIndex: 0`）で DOM 上にエミュレートする。ブラウザ CSS との完全一致は目標にしない。Canvas / Hayate と Z-Order が乖離しうるスタイルを設定した場合は開発時に警告する（拒否はしない）。警告対象は拡張可能な registry（`Z_ORDER_DIVERGENCE_PROPERTIES`）で管理し、現行は `opacity`、Element スタイルに追加された `transform` を含む。警告は dev のみ runtime `console.warn`（同一 element + property につきセッション 1 回）の単独 seam で行う。静的型チェック（`DomStylePatch`）は Tsubame Adapter が renderer 非依存（DOM/Hayate いずれのターゲットにもなり得る）であるため型分岐の利用箇所が成立せず不採用とした（ADR-0013、ADR-0006 の当該部分を supersede）。将来 ESLint によるファイル警告は理想だが初期スコープ外。
_Avoid_: SSR, hydration, Hayate HTML Mode（Hayate 不使用のため）, DomStylePatch による静的チェック（ADR-0013 で不採用）

## Integration Terms

**Hayate Protocol Contract**:
Hayate リポジトリ `proto/spec/` の JSON 契約定義群（JSON Schema で検証）。Tsubame は npm パッケージ `@torimi/hayate-protocol-spec` 経由で取り込み、`Tsubame/proto/generator/` から wire 定数と adapter vocabulary（`StylePatch`・`EventKind`・semantic mutation surface 等）を `Tsubame/proto/generated/` に生成し commit する。`setProperty`・`addEventListener` 購読 API は Renderer Protocol 独自 surface として Contract 外（codegen 対象外）。`resize` も spec codegen 対象外である点は同じだが、**もはや Renderer Protocol surface ではない** — host→adapter→core が所有し Tsubame は resize 経路から外れる（web は `hayate-adapter-web` の自己配線 ResizeObserver、android は native ループが `tree.set_viewport` を直接駆動。ADR-0080 を native へ延長, issue #475）。`HayateRenderer.resize()` と `RawHayate.on_resize` は撤去済み。Tsubame が将来 viewport を要する場合のみ spec Contract の API として供給する（当面は入れない）。
_Avoid_: wire only 生成、adapter 向け型の手書き維持、Contract から Renderer 実装まで生成する設計、`resize` を Renderer Protocol surface と呼ぶ説明、Tsubame が `raw.on_resize` を直接叩く設計

**apply_mutations**:
Hayate Renderer のフレームバッチ入口。Tsubame と Hayate の結合点の中心。

**Interaction Stream**:
（移行対象）Hayate Renderer 内の JS 側 event dispatch Module。`Element Document Runtime` 移管後は Hayate 内 dispatch に置き換え、Tsubame 側は host callback のみ残す。
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
Tsubame が Hayate Renderer 経由で利用する描画基盤。Tsubame は Hayate の内部実装にも**ランタイム/WASM adapter パッケージ（`hayate-adapter-web` 等）にも依存せず**、`@torimi/hayate-protocol-spec`（`proto/spec/*.json`）と自前定義の `RawHayate` ポート、`apply_mutations` / `poll_events` 契約だけを見る。具体 adapter は App が注入する（docs/adr/0004）。

**Hayabusa**:
Rust 側の長期構想。Tsubame は Hayabusa の JS 版ではない。

## Example Dialogue

> 「Tsubame は framework？」
> → 「違う。framework 固有ランタイムをそのまま使い、描画先を差し替える基盤」

> 「Hayate との結合点は？」
> → 「`@torimi/hayate-protocol-spec` と `apply_mutations` / `poll_events`」
