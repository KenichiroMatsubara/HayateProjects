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

**Interaction**:
hayate-core の interaction 状態機械を `ElementTree` から切り出した deep module（ADR-0122）。横断的 interaction state（focus / hover / active / press 位置 / `PointerGesture` / `PointerKind` / `InputModality` / 直近 pointer 位置・cursor / touch scroll）を所有し、`apply_intent(InteractionIntent)` 単一 seam を公開する。element 位相・layout 幾何・per-element `EditState`・scroll offset は所有せず狭い `InteractionTreeView` trait 越しに借り、Event と dirty mark は sink 経由で出す。ADR-0066 が runtime 所有とした interaction 状態の所在をこの module へ精緻化する。interface がそのまま test surface で、fake な tree view を与えて intent を流し込み状態と発火 Event を検証する。
_Avoid_: `ElementTree` の impl ブロックのまま散らす理解、Accessibility と並ぶ第二の state 所有者として切り出す設計（accessibility inbound は seam の consumer）、element 本体や layout を Interaction が所有する説明

**InteractionIntent**:
pointer / keyboard / accessibility / edit のすべてが合流する、`Interaction` の閉じた intent 語彙（flat dispatch 封筒）。`Edit(EditIntent)` arm は既存の `EditIntent`（ADR-0103）を**そのまま内包**し再定義しない。`Focus` / `Click` / `SetValue` / `ScrollToReveal` 等を並立 arm として持つ。各 Platform Adapter（pointer/key・AccessKit）と edit 経路が同一の値型を生産するため、intent 値はデータとして adapter を起動せず構築・テストできる（2 producer = 本物の seam）。`scroll_axis_to_reveal` 等の幾何は intent の裏（`Interaction` 実装）に置き、adapter は intent を出すだけ。
_Avoid_: `EditIntent` を広げて編集以外も飲ませる設計（`EditIntent` は edit 専用シームのまま・ADR-0103）、合成 pointer/key の replay で accessibility を駆動する設計、幾何計算を adapter 側に置く設計

**Caret Geometry（キャレット幾何）**:
text-input の幾何依存 edit 操作（縦移動 ↑↓・表示行 Home/End・point→byte の hit）が問い合わせる純粋 query seam（ADR-0122）。`line_of` / `x_of` / `byte_at_x_on_line` / `line_bounds` / `byte_at_point` 等を持ち、`EditState` の操作に注入される。`Taffy Projection`（block-box レイアウト）と直交する text 側の query で、実 adapter は Parley（`content_layout`）を包む 1 つ、test adapter は行→byte 範囲・byte→x の手書きテーブル。後者により縦移動等が full tree なしで純粋にテストできる。goal column（`edit.desired_x`）は `EditState` 側に残る。
_Avoid_: Parley `Layout` を直接 `EditState` に持たせる設計、`Taffy Projection` と同一視する理解、goal column を `Caret Geometry` 側に置く理解

**Tsubame**:
JS/TS 向けのレンダラーターゲット基盤。`Renderer Protocol`・`DOM Renderer`・`Hayate Renderer` を提供し、各フレームワーク固有ランタイムをそのまま持ち込む。
_Avoid_: signal ランタイム、フレームワーク本体

**Hayabusa**:
Hayate 上に構築する Rust フレームワーク構想。ADR-0045 により Hayate とは WIT 境界ではなく Rust crate 依存で接続する。現時点の開発優先は Tsubame 側であり、Hayabusa は長期構想として扱う。
_Avoid_: 現在の最優先実装対象

## Current Contracts

**Hayate Protocol Contract**:
Hayate リポジトリの `proto/spec/` に置く、Hayate-Tsubame 間の機械可読な契約。`opcodes`・`style_tags`・`event_kinds`・`element_kinds`・`unset_kinds`・`modifier_keys` 等を JSON で定義し、`proto/spec/schema/` の JSON Schema で検証する。正本は Hayate 側のみ。Hayate は `proto/generator/` から wire 定数・decode・encode（`codec.rs`）を `proto/generated/` に生成し commit する。Tsubame は npm パッケージ `@torimi/hayate-protocol-spec` 経由で Contract を取り込み、`Tsubame/proto/generator/` から wire 定数・TS encode（`codec.ts`）・adapter vocabulary（`catalog.ts` 等）を `Tsubame/proto/generated/` に生成し commit する。`style_tags` の `encodeFrom` は TS 向け入力変換規則（ADR-0055）。`style_tags` の `domCss`（tag→ブラウザ CSS 写像・`DOM_EXTRAS` 含む）も spec 正本で、`dom_style_mapper.rs`（Rust HTML Mode）と `catalog.ts`（TS DOM Renderer）を両側生成する（ADR-0070）。Renderer Protocol 独自の surface（`setProperty`・`addEventListener` 購読 API）と semantic mutation キュー（`HayateMutationPacket`）は Contract 外として手書きのまま残す。`resize` も Contract 外だが**もはや Renderer Protocol surface ではない** — viewport 追従は host→adapter→core（web は `hayate-adapter-web` の自己配線 ResizeObserver、android は native ループ）が `set_viewport` で所有し、Tsubame は resize 経路に存在しない（ADR-0080 を native へ延長, issue #475。当初の「resize は Renderer Protocol surface」記述の訂正）。
_Avoid_: YAML 正本、定数だけ生成して mutation/style encode を手書きし続ける運用、Contract から IRenderer 実装まで生成する設計

**apply_mutations**:
Tsubame `Hayate Renderer` が Hayate に渡すフレーム単位バッチ入口。シグネチャは `apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[])`。
_Avoid_: 個別 `element_set_*` 呼び出しを現行 hot path とみなす説明

**Hayate Mutation Packet**:
Tsubame `Hayate Renderer` と Hayate WASM の間で、フレーム内 mutation を `ops/styles/texts` の unified batch として発行する順序契約。HayateMutationPacket は semantic mutation の順序を保持し、flush 時に ADR-0052 の `apply_mutations` 境界へ一括エンコードする。
_Avoid_: DOM Renderer にも要求される汎用 Renderer Protocol、単なる opcode 定数表、slot 配列だけの狭い codec、TypeScript 側の順序管理を永続設計とみなす説明

**ListenerId**:
Hayate Element Document Runtime が `register_listener` で払い出す opaque な listener ハンドル。consumer は `Map<ListenerId, handler>` だけを保持する（Tsubame Hayate Renderer は JS 側、Hayabusa は Rust ランタイムインスタンス側）。App Host は drain だけを所有し `ListenerId`/handler を解釈しない — delivery は `DeliverySink` 経由で consumer の map へ届く（ADR-0117）。bubble path は runtime 側の責務。
_Avoid_: JS 側 element id × event kind ごとの handler map を長期設計とみなす説明、App Host が handler map を持つ理解

**Event Delivery**:
Hayate が bubble dispatch した結果。`poll_events()` が返す `{ listener_id, event }` エントリ。ADR-0018 export poll モデルの後継。App Host が `tick` 内で drain し、`DeliverySink` 経由で consumer へ push する（pull ではなく App Host 主導の push・ADR-0117）。raw platform input event そのものではない。
_Avoid_: raw event を host が decode して bubble する現行 Interaction Stream モデル、consumer が自前で drain（pull）する設計

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
`SceneGraph` を消費して描画結果を生成する実装単位。Web は CanvasKit / Vello / tiny-skia、Native は skia-safe / Vello を候補とし、`SceneGraph` の walk は `hayate-core` の `render_scene_graph` が一度だけ行う。
_Avoid_: Backend, Driver, adapter 内での SceneGraph walk

**CanvasKit Scene Renderer**:
Web 専用の `Scene Renderer`。CanvasKit の JS/WASM surface を Web Host / Platform Adapter から受け取り、共通 `SceneGraph` の描画意味論を CanvasKit API へ lower する。CanvasKit のロード・surface・context の所有や Tsubame への依存は持たない。
_Avoid_: Tsubame renderer、Native skia-safe 実装とのコード共有、HTML DOM renderer

**CanvasKit Command Buffer**:
CanvasKit Scene Renderer が共通 `SceneGraph` の walk から生成する Web 専用のフレーム単位描画バッチ。TypedArray の描画命令と Host 管理 resource id を wasm-bindgen の一回の境界越えで CanvasKit JS replay 層へ渡す。CanvasKit API の正本や Tsubame の Renderer Protocol ではない。
_Avoid_: DrawOp の新しい正本、CanvasKit API の直接 wire 化、命令ごとの Rust↔JS 呼び出し

**CanvasKit Resource Cache**:
Web Host が `ResourceId` から CanvasKit の font/image object へ解決する cache。Core が保持する font/image bytes を正本とし、CanvasKit object の生成・再利用・解放は Host が所有する。Native skia-safe の resource cache や CanvasKit object を Core/Tsubame へ公開しない。
_Avoid_: Core が SkTypeface/SkImage を所有する設計、Tsubame の resource cache、毎フレームの再登録

**Native Skia Renderer Family**:
Desktop・Android・将来の iOS で skia-safe を使う Native 側の renderer 群。今回の結線対象は Desktop/Android とし、iOS の surface・packaging は別スライスで扱うが、Cross-Renderer Parity の契約は同じ Native family に適用する。
_Avoid_: iOS を永久対象外とする理解、Web CanvasKit を Native family に含める説明

**ScenePainter**:
`hayate-core` 内部の walk callback interface。`fill_rect` / `draw_text_run` / `draw_image` と `push_transform` / `pop_transform` / `push_clip_rect` / `pop_clip` を実装する。host 向け公開契約ではない（ADR-0054）。
_Avoid_: Scene Renderer と混同、Platform Adapter の責務

**Cross-Renderer Parity（クロスレンダラ・パリティ）**:
異なる `Scene Renderer` が同じ `SceneGraph` と描画意味論に対して、共通の成功結果・入力検証・失敗分類を返すこと。CanvasKit（Web）と skia-safe（Native）の API・surface 固有の詳細や低レベル原因まで同一にすることは含まない。
_Avoid_: 実装共有、同一 API、ピクセル単位の完全一致、backend 固有障害の同一原因

**Validation Mode（検証モード）**:
`SceneGraph` の共通契約検証をコンパイル時に有効化または除外する構成。Debug は有効、CI・golden・調査版は `scene-validation` feature で明示的に有効、究極性能を優先する production release は無効とする。
_Avoid_: 実行時フラグだけで validator を止める構成、validator 無効時にも契約エラー保証があるという理解

**Render Host**:
surface 初期化、capability 判定、renderer 切替、資源寿命管理を担う描画オーケストレーション層。描画そのものは `Scene Renderer` が担う。tree/loop/event drain は持たず、それらは上層の App Host が所有して Render Host を駆動する。Tsubame Hayate Renderer（wire 経路）向けの web/native bootstrap は JS パッケージ `@torimi/hayate-host` が体現する：`createHayateWebHost(canvas)` が WebGPU プローブ・`Renderer Selection Policy`・WASM ロード・surface 取得を行い `RawHayate`(+frame-clock) を返し、`./native` の `createHayateNativeHost(raw)` が注入 RawHayate を vsync pump に結線する。host bootstrap は Tsubame の renderer パッケージには置かない（#477）。
_Avoid_: Platform Adapter, Backend, App Host（tree/loop/drain を持つ上層）との同一視、host bootstrap を Tsubame renderer パッケージに置く設計

**Renderer Selection Policy**:
どの `Scene Renderer` を優先し、どの条件で採用・不採用にするかを決めるルール。`Render Host` から分離する。
_Avoid_: host に埋め込まれた if 文連鎖

**Web Renderer Selection Order**:
Web の既定 `Scene Renderer` 試行順。CanvasKit を第一候補、環境エラー時の一方向 fallback として Vello、最後に tiny-skia を置く。共通契約エラーは fallback の理由にせず、その入力を失敗として扱う。
_Avoid_: Web と Native の同一順序、契約エラーを別 backend で黙って描く fallback、毎フレームの再選択

**Native Renderer Selection Order**:
Desktop / Android の既定 `Scene Renderer` 試行順。skia-safe を第一候補、boot 中の初期化失敗時だけ Vello を次候補とし、Android の skia-safe surface は GL を既定、GL 初期化失敗時は raster を使う（ADR-0149）。
_Avoid_: Web と同じ選択順、選択後の runtime fallback、Vello を Native の既定とする説明

**Renderer Selection Reason**:
`Render Host` が renderer を採用しなかった、または切り替えた理由を表す共通語彙。`WebGpuUnavailable` / `RendererInitFailed` / `SurfaceLost` / `CapabilityUnsupported` / `DisabledByPolicy` など。
_Avoid_: 場当たり的な文字列エラー

**Fatal Renderer Failure（致命的レンダラ障害）**:
CanvasKit または skia-safe の初回初期化に成功した後に発生する描画・surface・context エラー。選択済み renderer の状態を捨てて別 renderer を起動する runtime fallback / restart は行わず、App Host に terminal failure として報告する。初回 boot 中の候補試行や他 renderer の既存 fallback 方針とは別の状態である。
_Avoid_: 選択後の自動 backend 切替、context loss の透過的復旧、初回 init failure との同一視

## Runtime Boundaries

**App Host（アプリホスト）**:
プラットフォーム非依存の最上位協調層であり、mount 先。`ElementTree` 実体の所有・フレームループ・`Event Delivery` の drain・Font ロードを担い、内部で **Render Host**（描画オーケストレーション・ADR-0068）を駆動する。OS フレームループは所有せず、`tick(timestamp_ms)` を公開して Platform Front から毎フレーム呼ばれる。フレームを起こす入口は単一の `request_redraw`（= wake）で、継続（進行中 animation / `visual_dirty`）は App Host、入力到着は Platform Adapter、非同期 signal 変化（Resource / Store / timer）は consumer が同じ入口を叩く。frame の意味順序は delivery drain → consumer flush → commit/present で、App Host 自身が `Idle` / `Prepared(frame_id)` の状態機械として順序を保証する。in-process projection は二相を単一 `tick(timestamp_ms)` 内で連続実行し、wire projection は Rust の可変所有を JS callback へ再入させず二相間で制御を返す（ADR-0150）。pending が無ければ idle に落ちる。Hayabusa（in-process Rust）と Tsubame Hayate Renderer（wire 経路）は共にこの App Host へ mount し、tree/loop/drain/描画/font の意味を自前で再定義しない。Platform Adapter は App Host の裏に薄い trait（`Surface` / `FontFetcher` ＋ IME・入力）として収まり、Hayabusa は Platform Adapter も Platform Front も直接触らない。DOM input、Canvas resize、IME、clipboard、adaptive render scale など platform 固有の前後処理は Platform Adapter の深い frame façade に集約し、App Host に持ち込まない。ADR-0068 の共有層を実体化・拡張したもの。
_Avoid_: Render Host（描画専用の下位層）との同一視、Platform Adapter / Platform Front との同一視、OS フレームループを App Host が所有する設計、consumer ごとのフレーム trait を受ける設計、wake 入口を源ごとに分ける設計、tick 外で mutation を出す flush 経路、フレームワーク/Reconciler/Component tree、consumer ごとに別ホストを持つ設計

**Committed Frame（確定フレーム）**:
Core の frame commit が返す renderer-ready かつ platform-free な不変 view。`SceneGraph`、frame layer 順序、content/chrome dirty layer、scroll layer の compositor 入力、pending visual work の有無を一括で運ぶ。App Host はこれだけを `PresentTarget` へ渡し、Render Host に `ElementTree` を公開しない。background、surface size、render scale と backend 用 scroll geometry は platform/renderer projection が補う（ADR-0150）。
_Avoid_: `SceneGraph` だけを完全な frame output とみなす理解、`ElementTree` 全体を renderer へ渡す設計、DOM/Canvas 固有値を含める設計

**Platform Front（プラットフォームフロント）**:
OS のフレームループ（web `requestAnimationFrame` / Android `Choreographer`）を所有する per-platform な駆動・入口層。App Host を構築して `request_redraw` クロージャを渡し、毎フレーム `App Host::tick(timestamp_ms)` を呼ぶ。web binding / native binding が体現する。App Host の裏の trait として収まる Platform Adapter（`Surface`/`FontFetcher`/IME/入力）とは別軸で並立する（ADR-0117）。
_Avoid_: Platform Adapter との同一視、App Host がこれを兼ねる理解、継続フレーム判定をここが持つ理解（判定は App Host、スケジューリングのみ Platform Front）

**DeliverySink**:
consumer が mount 時に App Host へ渡す Event Delivery の受け口。App Host は `poll_events()` が返す `{listener_id, event}` batch の drain を所有し続け、delivery が空でも毎フレームこの flush 点を駆動する。consumer は handler 由来・非同期由来（Resource / Store / timer）を問わず reactive graph をここで flush する。in-process projection は drain 済み batch を同期 callback へ渡し、handler 実行・flush・Element Layer mutation が return 前に完了してから commit/present へ進む。wire projection は同じ意味順序を二相 frame contract で表し、preparation が batch を返して Rust 呼び出しを終え、JS の handler 実行と mutation flush 後に commit/present する（ADR-0150）。
_Avoid_: consumer が `poll_events()` を自前 pull する設計（drain 所有が App Host から漏れる）、delivery 非空のフレームだけ呼ばれる理解、App Host が `ListenerId`/handler を解釈する理解、tick 外で flush・mutation する経路、raw event を運ぶ理解

**Platform Adapter**:
単一プラットフォーム（web / android / ios / 将来の macos 等）の **leaf** 層。surface 生成 glue・raw event 配線・`ImeBridge` 実装・`Capability` の platform 実装といった**完全に platform 固有な glue だけ**を持つ。platform-free な共通ロジック（後述）は Core 所有、family 統一 capability は Family Adapter 所有。ネイティブが自動供給するイベント（Web の DOM pointer/wheel/resize/touch 等）は host 側 glue を介さず Platform Adapter 自身が購読・変換する（ADR-0080）。アダプタ間で windowing/event-loop glue は共有しない（ADR-0087/0114）。
_Avoid_: Renderer, Host, host 側 glue コードでの DOM イベント購読を前提とした説明、platform-free ロジックや family capability を leaf に持たせる説明、web/mobile/desktop を leaf と同列に並べる説明

**Family Adapter**:
複数の leaf を束ねる中間層（`mobile` = android + ios / `desktop` = macos + windows + linux）。存在理由は **family 内で統一できる platform-bound capability（audio 等）を単一 facade で上位へ供給する**こと。ビルド時 `cfg(target_os)` で片方の leaf 実装をリンクする facade であり、ランタイム dispatch ではない。capability の**契約（trait）は Core 定義**で、Family Adapter は実装の束ねと family facade のみ持つ。`web` は単一 platform（family of 1）なので Family Adapter を持たず leaf が直接置かれる。
_Avoid_: Flutter platform channel / RN bridge 的なランタイム機構、family adapter が capability 契約の正本を持つ説明、family adapter に windowing/surface を持たせる説明、web に親 family を作る説明

**Capability**:
各 OS のネイティブ API 呼び出しが**必須**な機能（audio / clipboard / notification / haptics / file picker 等）。共通度で三段階に分類する — 全 platform 共通（`platform/common/`）・family 共通（`platform/mobile/`・`platform/desktop/`）・leaf 固有。**契約（trait）は常に Core が所有**（`ImeBridge`/`Surface`/`FontFetcher` と同型・ADR-0068/0069）、実装は leaf。共通 API への昇格は原則 2 実装が揃ってから（ADR-0068 の投機 seam 戒め）だが、Flutter/RN の prior art で variation が確定済みかつ ADR-0012 で確定ターゲットの desktop 枠は前払い可。三段階の振り分け規則・Flutter/RN taxonomy からの分類例・「契約は Core / 昇格は 2 実装から / 借りるのは taxonomy のみで機構（channel/bridge）は借りない」の規律の正本は [`crates/platform/README.md`](crates/platform/README.md)（grouping doctrine）。
_Avoid_: platform-free な共通ロジック（touch gesture / surface 状態機械 / IME 増分 = Core 所有）と混同する説明、capability 契約を adapter 側に置く説明

**Capability Scaffold**:
実機実装の前段として、capability を「Core trait ＋ android/ios 両 leaf stub ＋ mobile facade」で先に型として存在させ、呼ぶと typed な未実装エラー（`Unimplemented`）を返す breadth-first な状態。契約の形は Flutter の `platform_interface`（未実装は既定でエラーを throw）を写し、Rust では throw を `Result::Err` へ写像する。両 leaf stub が揃うことで「昇格は 2 実装から」ゲートの**意図**（1 platform 決め打ちで契約形を誤らない）を満たす — contract の形が実機実装で変わりうる点は受容する（ADR-0119）。
_Avoid_: 空 trait の先置きと同一視（scaffold の stub は throw する契約を持つ）、panic/`unimplemented!()` で表す理解（typed な `Err` を返す）、機構（channel/bridge）まで Flutter から借りる理解（借りるのは taxonomy と throw-by-default な契約形のみ）、「完璧な契約設計」と捉える理解（狙うのは網羅・型付き・ちゃんとエラーまで）

**Stream Capability（ストリーム capability）**:
「現在値の単発取得 ＋ 状態変化イベントの連続供給」が本質の capability（battery / connectivity / geolocation / sensors = ADR-0119 の wave-2）。一発応答（wave-1）と契約の形が違う。Core trait が `query()`（現在値・`&self`・`Result<T, CapabilityError>`・wave-1 同型）と `subscribe()`（変化ストリーム・`&mut self`・`Result<Subscription, _>`）の 2 メソッドを持ち、契約が保証するのは「`subscribe` が変化を流す」ことだけ（初期値が要れば `query` 併用）。`EventDelivery` には乗せず（element ターゲットでない）専用契約とし、変化通知は ADR-0117 の「非同期 signal 変化」wake 源に合流して tick の単一 flush 点で reactive flush する（ADR-0120）。
_Avoid_: `EventDelivery`/`DeliverySink` に乗せる理解（element/bubble 前提を曲げる）、query と subscribe を 1 メソッドに畳む設計、subscribe が必ず初期値を出すと契約に書く理解、一発応答 capability（wave-1）と同一視

**Capability Subscription（購読ハンドル）**:
`Stream Capability` の `subscribe()` が返す **RAII ハンドル**で、購読の生存そのもの。`poll_changes() -> Vec<T>` で蓄積された変化を consumer が**フレームの flush 点で drain** する（`poll_deliveries` と同型・値コールバックは契約に持たない）。`Drop` で leaf（Platform Adapter）が native 登録を解除する（**契約と Drop 意味論は Core、解除の native 手続きは leaf**）。解除失敗は best-effort で握り潰す（`Result` を取らない）。所有者は consumer（アプリ／Hayabusa ランタイム側）で、unmount でハンドルが drop すれば購読も終わる。値の届き方は **wake = leaf push（`request_redraw`・ADR-0080/0117）／ value = consumer pull（drain）** のハイブリッドで、threading marshaling とバッファは leaf に隠れる（ADR-0120）。
_Avoid_: 明示 `unsubscribe(id)` ペア（呼び忘れで native listener リーク）、pure poll（毎フレーム drain で idle 落ちを壊す）、値コールバック `FnMut(T)` を Core 契約に置く設計、解除に `Result` を期待する理解、buffer/marshaling を Core が持つ理解

**Tsubame Adapter**:
`tsubame-solid` / `tsubame-vue` / `tsubame-react` の総称。各フレームワーク固有ランタイムを維持したまま、レンダリング先だけを `Renderer Protocol` に向け替える。
_Avoid_: shared component runtime, unified signal runtime

**Accessibility Mirror（アクセシビリティミラー）**:
Canvas モードで `@torimi/hayate-host` が `<canvas>` の兄弟に建てる不可視 ARIA DOM。Core の `poll_accessibility()`（AccessKit `TreeUpdate` の JSON）を role/name/value/bounds に 1:1 で写し、ブラウザのアクセシビリティツリー＝Playwright `getByRole`/aria-snapshot に Canvas の中身を可視化する（第一目的は AI による自動テスト）。v1 は読み取り専用（`opacity:0` ＋ `pointer-events:none`、駆動は座標経由）で Chrome 限定（ADR-0124、ADR-0041 を Chrome スコープで front-run）。
_Avoid_: 実 AT 報告（UIA/NSAccessibility）専用と捉える理解、Tsubame レンダラの責務とする理解、interactive（`getByRole().click()` を Core へ往復）を v1 が持つ理解、Canvas を実 DOM に置き換える設計との混同

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
> → 「`@torimi/hayate-protocol-spec`（`proto/spec/*.json`）と、それに基づく `apply_mutations` / `poll_events` だよ」

> 「Tsubame は signal ランタイムなの？」
> → 「違う。各フレームワークの既存ランタイムをそのまま使い、レンダリング先だけを差し替える基盤だよ」

> 「Hayabusa は消えたの？」
> → 「消えていない。Rust 側の長期構想として残っている。ただし現状の開発優先は Tsubame 側」
