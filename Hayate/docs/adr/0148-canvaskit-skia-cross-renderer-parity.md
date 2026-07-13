# CanvasKit と skia-safe の Cross-Renderer Parity

**Status: accepted**

Web の CanvasKit Renderer と Native の skia-safe Renderer は、API と surface が異なるため別実装として導入する。ただし両者は共通の `SceneGraph`、描画意味論、入力検証、失敗分類を共有し、同じ契約違反は同じ共通契約エラーとして失敗させる。CanvasKit の context 喪失や skia-safe の EGL 初期化失敗のような環境エラーは backend 固有の原因を許容しつつ、`RendererInitFailed` / `SurfaceLost` など共通カテゴリへ正規化する。

CanvasKit の JS/WASM モジュール、surface、context は Web Host / Platform Adapter が所有する。Tsubame の renderer と共通 Core は CanvasKit を依存先として知らず、Web の `CanvasKit Scene Renderer` は Host が供給する opaque な描画資源だけを受け取る。

CanvasKit への Rust↔JS 呼び出しは描画命令ごとに行わず、共通 walk から Web 専用 `CanvasKit Command Buffer` を生成し、フレーム単位で一括 replay する。これにより CanvasKit API の詳細を replay 層へ閉じ、wasm-bindgen 境界の往復を描画命令数に比例させない。

`RenderFont` / `RenderImage` の bytes は Core が正本として保持する。Web Host は `ResourceId` から CanvasKit の font/image object へ解決する `CanvasKit Resource Cache` を所有し、command buffer は新規 payload と ID の参照だけを渡す。Native skia-safe は同じ bytes から自身の resource を構築し、CanvasKit object を共有しない。

Web の `Renderer Selection Policy` は `CanvasKit → Vello → tiny-skia` の一方向順序とする。CanvasKit を既定とし、CanvasKit のロード・context 初期化・surface 喪失など環境エラーだけを次候補へ fallback する。共通契約エラーは別 renderer で隠さない。

CanvasKit は初回フレームの前に Web Host がロード・初期化する。先に別 renderer で描画して後から CanvasKit へ切り替える progressive boot は採らず、初期化が失敗した場合だけ同じ boot 中に Vello、次に tiny-skia を試行する。

初回選択に成功した後の描画・surface・context エラーは `Fatal Renderer Failure` として扱う。CanvasKit または skia-safe が runtime に失敗しても、別 renderer への fallback や非同期 restart は行わず、App Host が terminal failure としてアプリを停止させる。fallback は初回 boot 中の未選択候補に限る。

Native Skia Renderer Family は Desktop・Android・将来の iOS を含む。今回の実装結線は Desktop/Android に限定し、iOS の surface・packaging は別スライスで行うが、iOS も skia-safe を使い同じ Cross-Renderer Parity 契約へ参加する。

本決定の変更対象は描画 backend とその Host 結線だけである。既存の Core の layout、input、font、SceneGraph lowering、Renderer Protocol は変更しない。CanvasKit/skia-safe は既存の `SceneGraph` / `TextRunData` / resource bytes を消費するだけで、新しい意味論や処理経路を導入しない。

契約検証は Core の incremental validator が担い、初回 SceneGraph は全体、変更された retained subtree のみ再検証し、不変フレームは検証を省略する。Validation Mode は compile-time とし、Debug は有効、CI・golden・調査版は `scene-validation` feature で有効化し、究極性能を優先する production release は validator のコードと分岐を除外する。validator を無効にしたビルドでは共通契約エラー保証を提供しない。

## Consequences

- CanvasKit/skia-safe の painter 実装は共有せず、SceneGraph walk と契約だけを共有する。
- CanvasKit の JS API は Web 専用 command buffer replay 層に閉じ、Tsubame / Core へ漏らさない。
- 共通契約違反と backend 環境障害の観測を分離できる。
- retained scene の変更追跡と、CI での validator 有効ビルドが必要になる。
- production の最大性能ビルドでは validator の実行時コストをゼロにできる。
- backend の検証は既存 SceneGraph fixture に対する CanvasKit/skia-safe 固有 golden・command buffer・初回選択・terminal failure に限定し、既存の Core 意味論テストは変更しない。
