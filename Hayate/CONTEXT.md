# Hayate Glossary

Hayate / Tsubame 周辺で現在使う語彙だけをまとめる。現行仕様の根拠は [`docs/spec.md`](/C:/Users/pinara/Desktop/myapps/HayateProjects/Hayate/docs/spec.md) と各 ADR に置き、ここには古い構想や詳細設計を書き込まない。

## Product Names

**Hayate**:
GPU 描画を担う Rust/WASM 側の UI 基盤。`Element Document Runtime`（element tree・listener dispatch・基本 scroll 等）と `SceneGraph` を保持し、`Scene Renderer` を通じて描画する。
_Avoid_: layout/GPU のみの paint server としてのみ説明する、Hayabusa 中心の説明

**Element Document Runtime**:
`hayate-core` Element Layer 内の軽量 document engine。element tree・listener 登録・bubble/non-bubble dispatch・scroll-view の基本 offset 更新・focus 等を担う。**interaction 状態機械（focus/active/hover の単独所有と `on_pointer_*`/`on_key_down`/`on_wheel`/`on_text_input`/`on_composition_*` の入力 surface）も runtime が持つ**（ADR-0066）。Canvas/HTML 経路の **Canonical Tree（描画・layout・hit-test の単一正本）**（ADR-0062 が ADR-0057 の核を継承。tsubame-solid のみ構造専用 shadow tree を別途保持）。`:hover` / `:active` / `:focus` を Hayate CSS の一部として保持し、render 時に effective style へ合成する（ADR-0056）。Platform Adapter は raw 入力（pointer / wheel / EditContext 等）をここへ渡す。`hayate-adapter-web` 等は input 変換と描画 flush のみ。host は dispatch 結果を `poll_events()`（または後継 export）で受け取り、listener id に紐づく callback を実行する。慣性 scroll は担わない。
_Avoid_: adapter 層ごとの document semantics、Tsubame 側 bubble、Tsubame 側 shadow tree、Hayate から host への import callback（ADR-0018 参照）

**ElementEngine**:
`ElementTree` 内部の private module（`element/engine.rs`）。`structure_dirty` / `shape_dirty` / `fonts_dirty` の dirty 集合を集約・保管し（store/merge）、`ElementTree::commit_frame()`（dirty 解決＋layout settling、`LayoutPass::run()` 相当）から呼ばれる（ADR-0075）。「どの要素を/どこまで dirty にするか」の reach 意味論は持たず、`Invalidation`（`visual_invalidation.rs`）が `classify` / `step_reach` で決める。`tree.rs` の `element_set_*` は位相を読んで `ElementContext` を組み「何が変わったか」を報告するだけ（issue #238）。`fonts_dirty` / `viewport_dirty` のような非 prop 駆動の無効化は従来どおり engine への直接 mark。
_Avoid_: ElementEngine が ElementTree を所有/置換する新 public 型として説明する、DocumentEngine という名称（Canonical Tree と語が衝突する）、reach 分類を engine 内に持つ説明

**ElementContext**:
要素の位相（`kind` / `is_ifc_root` / `has_text_parent`）だけを写し取った値オブジェクト（`visual_invalidation.rs`、private）。`tree.rs` が live tree から組み、`classify(prop, ctx)` と `step_reach(reach, parent_ctx, child_ctx)` という純関数へ渡す。これにより無効化の意味論（dirty 種別＋reach の決定、reach 伝播）が `ElementTree` を起動せず単体テスト可能になる（reach テーブルがそのまま test surface）。`classify` が「何を/どこまで dirty にするか（WHAT）」、`step_reach` が「reach がどの子へどこまで届くか」を単一ソースで決め、scene lowering の retained walk と patch-root 探索の双方が `step_reach` を共用する（issue #238、ADR-0086 維持）。
_Avoid_: `ElementContext` が要素本体や dirty 集合を所有するという説明、reach 伝播を walk 側に二重実装する設計

**Tsubame**:
JS/TS 向けのレンダラーターゲット基盤。`Renderer Protocol`・`DOM Renderer`・`Canvas Renderer` を提供し、各フレームワーク固有ランタイムをそのまま持ち込む。
_Avoid_: signal ランタイム、フレームワーク本体

**Hayabusa**:
Hayate 上に構築する Rust フレームワーク構想。ADR-0045 により Hayate とは WIT 境界ではなく Rust crate 依存で接続する。現時点の開発優先は Tsubame 側であり、Hayabusa は長期構想として扱う。
_Avoid_: 現在の最優先実装対象

## Current Contracts

**Hayate Protocol Contract**:
Hayate リポジトリの `proto/spec/` に置く、Hayate-Tsubame 間の機械可読な契約。`opcodes`・`style_tags`・`event_kinds`・`element_kinds`・`unset_kinds`・`modifier_keys` 等を JSON で定義し、`proto/spec/schema/` の JSON Schema で検証する。正本は Hayate 側のみ。Hayate は `proto/generator/` から wire 定数・decode・encode（`codec.rs`）を `proto/generated/` に生成し commit する。Tsubame は npm パッケージ `@hayate/protocol-spec` 経由で Contract を取り込み、`Tsubame/proto/generator/` から wire 定数・TS encode（`codec.ts`）・adapter vocabulary（`catalog.ts` 等）を `Tsubame/proto/generated/` に生成し commit する。`style_tags` の `encodeFrom` は TS 向け入力変換規則（ADR-0055）。`style_tags` の `domCss`（tag→ブラウザ CSS 写像・`DOM_EXTRAS` 含む）も spec 正本で、`dom_style_mapper.rs`（Rust HTML Mode）と `catalog.ts`（TS DOM Renderer）を両側生成する（ADR-0070）。Renderer Protocol 独自の surface（`setProperty`・`addEventListener` 購読 API・`resize`）と semantic mutation キュー（`HayateMutationPacket`）は Contract 外として手書きのまま残す。
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
Hayate が保持する描画用の retained graph。element→scene lowering は dirty-gated incremental で、未 dirty の subtree はフレーム間で再利用する（ADR-0086）。UI フレームワークやコンポーネントツリーそのものではない。
_Avoid_: Virtual DOM, Component Tree

**Element Anchor（要素アンカー）**:
`NodeKind::ElementAnchor` — element ごとの構造専用 retained Node。詳細はルート `CONTEXT.md` の Rendering Terms。

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
単一プラットフォーム（web / android / ios / 将来の macos 等）の **leaf** 層。surface 生成 glue・raw event 配線・`ImeBridge` 実装・`Capability` の platform 実装といった**完全に platform 固有な glue だけ**を持つ。platform-free な共通ロジック（後述）は Core 所有、family 統一 capability は Family Adapter 所有。ネイティブが自動供給するイベント（Web の DOM pointer/wheel/resize/touch 等）は host 側 glue を介さず Platform Adapter 自身が購読・変換する（ADR-0080）。アダプタ間で windowing/event-loop glue は共有しない（ADR-0087/0114）。
_Avoid_: Renderer, Host, host 側 glue コードでの DOM イベント購読を前提とした説明、platform-free ロジックや family capability を leaf に持たせる説明、web/mobile/desktop を leaf と同列に並べる説明

**Family Adapter**:
複数の leaf を束ねる中間層（`mobile` = android + ios / `desktop` = macos + windows + linux）。存在理由は **family 内で統一できる platform-bound capability（audio 等）を単一 facade で上位へ供給する**こと。ビルド時 `cfg(target_os)` で片方の leaf 実装をリンクする facade であり、ランタイム dispatch ではない。capability の**契約（trait）は Core 定義**で、Family Adapter は実装の束ねと family facade のみ持つ。`web` は単一 platform（family of 1）なので Family Adapter を持たず leaf が直接置かれる。
_Avoid_: Flutter platform channel / RN bridge 的なランタイム機構、family adapter が capability 契約の正本を持つ説明、family adapter に windowing/surface を持たせる説明、web に親 family を作る説明

**Capability**:
各 OS のネイティブ API 呼び出しが**必須**な機能（audio / clipboard / notification / haptics 等）。共通度で三段階に分類する — 全 platform 共通（`platform/common/`）・family 共通（`platform/mobile/`・`platform/desktop/`）・leaf 固有。**契約（trait）は常に Core が所有**（`ImeBridge`/`Surface`/`FontFetcher` と同型・ADR-0068/0069）、実装は leaf。共通 API への昇格は原則 2 実装が揃ってから（ADR-0068 の投機 seam 戒め）だが、Flutter/RN の prior art で variation が確定済みかつ ADR-0012 で確定ターゲットの desktop 枠は前払い可。
_Avoid_: platform-free な共通ロジック（touch gesture / surface 状態機械 / IME 増分 = Core 所有）と混同する説明、capability 契約を adapter 側に置く説明

**Tsubame Adapter**:
`tsubame-solid` / `tsubame-vue` / `tsubame-react` の総称。各フレームワーク固有ランタイムを維持したまま、レンダリング先だけを `Renderer Protocol` に向け替える。
_Avoid_: shared component runtime, unified signal runtime

## Style Resolution Terms

**Viewport Condition**:
スタイルプロパティの値が、ルートサーフェスの幅・高さに対する `min-width`/`max-width`/`min-height`/`max-height`（px固定、1エントリ内はAND評価）の組み合わせに応じて切り替わるバリアント。`effective_visual` resolver が継承（ch1+ch2）→ 自身 → pseudo (`focus<hover<active`) に続く解決軸として扱う。同一プロパティに複数の Viewport Condition が同時マッチする場合は宣言順で後勝ち（CSSの `@media` カスケードに準拠）。
_Avoid_: 要素自身の `min-width`/`max-width` style tag（box constraint としての CSS `min-width`/`max-width` プロパティ）と同一視する説明、Container Query（要素自身の確定サイズに基づく条件。現時点ではスコープ外）

**Element-Kind UA Default（要素種別 UA 既定）**:
要素種別がブラウザ UA スタイルシート相当として持つ既定値で、core が単一正本から供給し全レンダラーが一致する。cursor（`<input>`→I-beam / `<button>`→pointer・ADR-0105）と layout（`button`=内容を縦中央 / `text-input`=フォント感応の既定幅・ADR-0109）が該当。解決順は常に **明示スタイル > element-kind UA 既定 > 全体既定**。値はブラウザ UA を写すが Canvas が正準（core がセマンティクスを定義）で、「DOM だけ動いていた偶然」（ADR-0002）とは区別される正当な種別セマンティクス。
_Avoid_: レンダラーごとに再宣言する既定、ブラウザ既定挙動への無条件委譲、authoring 必須として既定を持たない設計

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
