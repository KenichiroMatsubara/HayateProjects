# Android Chrome vello モードのカクつき対策（コンポジタ配線 epic 以外）

2026-07-02。ブランチ `claude/android-chrome-vello-rendering-ls1rzp`。
`docs/perf-android-rendering-diagnosis.md`（2026-07-01）の続編。

前提: 主因「毎フレーム全画面・物理解像度・フルシーン再ラスタ」は epic #631
（sub #632–#636）が、画像の Blob 不安定は #630 が対処中。本書は **それら以外**に
Android Chrome（web Canvas Mode / vello バックエンド）でフレームを落とす要因を
コード精査＋ホスト計測で洗い出し、対策を優先順で示す。

## 計測方法（フィードバックループ）

実機は使わず、web の毎フレーム経路（`HayateRenderer.frame` → `render()` →
`poll_events()`、および Accessibility Mirror の独立 rAF tick）を構成する仕事を
ホスト（x86_64, `--release`）で分解計測する。既存プローブに加え、新プローブを常設した:

```
HAYATE_PERF_PROBE=1 cargo test --release -p hayate-demo-fixtures --test a11y_perf_probe -- --nocapture
HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-vello --test perf_probe -- --nocapture
```

シーンは実アプリ相当の共有 fixture `hayate_demo_fixtures::tasks_tree`
（980x1060、a11y ノード 126、AccessKit JSON 19.9 KB）。

計測結果（p50, x86_64 release）:

| 項目 | 時間/フレーム |
|---|---|
| `accessibility_update()`（全ツリー walk 再構築） | 0.028 ms |
| `accessibility_update()` + `serde_json::to_string`（= `poll_accessibility` の中身） | 0.086 ms |
| `tree.render` アイドル（dirty ゲート済み・対比用） | 0.036 ms |

実機（CPU 3〜5 倍遅い＋WASM ~2 倍）換算で `poll_accessibility` は **0.5〜1 ms/フレーム**。
これに JS 側（下記）が乗る。

## 要因のランキングと対策

### 1. Accessibility Mirror: 常時 rAF ループ＋毎フレーム全ツリー再構築・全量 JSON（ADR-0124 / ADR-0126 違反）

`attachAccessibilityMirror`（`Hayate/host/src/accessibility-mirror.ts`）は自前の
rAF ループを**無条件に再武装**し、毎 vsync `raw.poll_accessibility()` を呼ぶ。
その中身（`crates/core/src/element/accessibility.rs::accessibility_update`）は
**dirty ゲートなし**の全ツリー walk＋全ノード構築＋serde_json 全量シリアライズ。

体感への効き方が二段ある:

- **スクロール / transition 中**: bounds が毎フレーム変わるので「前回 JSON と同一なら
  スキップ」の早期 return が**効かない**。毎フレーム、Rust 側 0.5〜1 ms に加えて
  JS 側で 20 KB の `JSON.parse` ＋ 126 要素の DOM style 書き換え（position/width/height）
  ＋ミラー DOM のスタイル・レイアウト無効化が、**描画と同じ main スレッド**で走る。
  中位機では数 ms/フレーム級で、epic がラスタをゼロにした後も体感ジャンクとして残る。
- **アイドル時**: フレームは出ないが rAF ループ自体は永久に回り、毎 vsync
  JS→WASM 呼び出し＋全ツリー walk＋シリアライズ＋20 KB 文字列比較を実行する。
  ADR-0126「idle でフレームゼロ」の趣旨（発熱・電池、PRD #607 User Story 3）に反する。
  実機ベースライン計測（#616）は「新規フレームが出ない」ことしか見ないため、
  この常時 JS 実行は検出をすり抜ける。

対策（効果順）:

1. core に **a11y 世代カウンタ（dirty ゲート）**を足す。ツリー構造・role/label/value・
   layout・focus・scroll のいずれかが変わったフレームだけ `accessibility_update` を
   再構築し、変化なしなら `poll_accessibility` が `null`（もしくは前回文字列）を即返す。
   dirty 追跡は ADR-0099 の既存 routing に相乗りできる。
2. ミラーの**独立 rAF ループを廃止**し、レンダラのフレームに相乗りさせる
   （`HayateRenderer.frame` の末尾 or `createHayateWebHost` が両者を束ねる）。
   idle でループが完全に止まり、wake 経路はレンダラと共有になる。
3. **スクロール中の bounds 更新をスロットル**する（スクロール静定後に 1 回反映）。
   ブラウザ自身の accessibility tree も非同期・低頻度更新であり、AT の意味論を
   壊さない。scroll offset 由来の bounds 変化だけを「後回し可」に分類できると理想。

### 2. エンジン一式が main スレッド（ADR-0128 の web 半分が未配線）

`createHayateWebHost` は main スレッドで WASM を起動し、`HayateRenderer.frame` が
flush → layout → lowering → vello エンコード → GPU submit → present を 1 つの rAF
コールバックで実行する。Tsubame の reactivity・delivery ハンドラ・GC・フォント/画像
デコード・上記ミラーがすべて同じスレッドを奪い合う。**JS の long task = 即フレーム落ち**。

ADR-0128 の web 近似形（OffscreenCanvas＋単一 Worker）は `Hayate/host/src/worker-host.ts`
（`MainThreadShim` / `WorkerEngineDispatcher`）として**scaffold とテストのみ存在し、
実 boot 経路のどこからも使われていない**（利用箇所は worker-host.test.ts のみ）。

対策: #636（web レイヤ planning 配線）の後続として「OffscreenCanvas＋Worker への
エンジン移設」を issue 化する。PRD #607 どおり計測ゲート付き（native と違い web は
コミット済みでない）。IME ブリッジ（EditContext は main 限定）は shim 済みの seam が
worker-host.ts に既にある。

### 3. 適応的レンダースケール（ADR-0129 / `RenderScaleGovernor`）が未配線

状態機械（`crates/core/src/render_scale.rs`）はホストテスト済みだが、
`RenderScaleGovernor` / `effective_content_scale` を消費するバックエンドが**ゼロ**
（全 hit が core 内。コンポジタ配線 epic と同じ「絵に描いた」状態）。DPR≈3 の
中位機で熱・負荷逼迫しても、フル物理解像度で描き続ける。

epic 完了後も「dirty レイヤの再ラスタ」は物理解像度で走るため、逼迫時の保険として
依然有効。またこれが入らない限り、劣化しきい値チューニング（#619）は着手できない。

対策: web の frame ループで rAF timestamp 差分から `note_frame` を駆動し、スケール
変更時に `ViewportMetrics` へ render_scale を乗せて buffer resize（`backend.resize`）
とヒットテスト座標（`hit_test_logical`）へ配線する。CSS サイズは不変なのでブラウザが
拡大表示する。

### 4. 画像デコードが main スレッド同期

`canvas.rs::fetch_image` は `image::load_from_memory`（WASM 内 CPU デコード）を
そのまま実行する。大きめの画像 1 枚で数十〜数百 ms の**スパイクジャンク**になる
（スクロール中に遅延ロードが解決すると 1 フレーム丸ごと落ちる）。

対策: ブラウザの `createImageBitmap`（オフスレッドデコード）でデコードし RGBA を
転写する。Worker 化（対策 2）後は worker 側デコードでも可。#630（Blob 安定化）と
併せて画像経路の毎フレーム/スパイク両方が消える。

### 5. 起動・初回操作のシェーダコンパイルスパイク（web に warmup が無い）

`VelloSceneRenderer::new`（`pipeline_cache: None`）はパイプラインを init 時に生成
するが、ブラウザ（Dawn）は内部で非同期コンパイルするため、**初回 dispatch 時に
ブロック**しうる。ADR-0130a の warmup は #633 で compositor パイプラインに入るが、
vello 本体のパイプラインは対象外。初回タップ・初回スクロールのカクつき（PRD #607
User Story 4）に効く。

対策: init 直後に 1x1 のダミー `render_to_texture` を 1 回流して全 variant を
実コンパイルさせる（web は起動時のみ・低優先）。

### 6. 微小だが積もる per-frame アロケーション / 強制レイアウト読み

- vello `Scene::new()` を毎フレーム生成（`Scene::reset()` の再利用で churn 削減可）。
- `poll_events()` / `encode_deliveries` が毎フレーム JS Array を生成（GC 圧）。
- `sync_edit_context` がキーボード表示中に毎フレーム `getBoundingClientRect()`
  （強制レイアウト読み。ミラーの DOM 書き換えと同フレームで layout thrash になり得る）。

いずれも単体では小さいが、GC / layout スパイクの裾を作る。対策 1・2 の後に
まとめて削るのが効率的（低優先）。

### 棄却した仮説

- ポインタ配線の browser gesture 干渉（`touch-action: none` 設定済み・listener は
  バッファ＋フレーム drain で健全。#623/#626 で wake / 慣性も修復済み）
- on-demand 契約違反によるアイドル再描画（`has_pending_visual_work` ゲートは正しく
  配線済み。残る常時ループは対策 1 のミラーのみ）
- present 経路の frame pacing（offscreen に描いてから acquire→blit→present の順序は
  surface hold 時間最小で健全。blit の追加 1 パスは #633/#636 の compositor が置換する）

## 再計測

`crates/demo-fixtures/tests/a11y_perf_probe.rs` を env ゲート付きで常設した。
対策 1（dirty ゲート）の前後で同コマンドを流せば、「変更なしフレームで
`accessibility_update` が走らない」ことをホストで確認できる。スクロール中の JS 側
コスト（JSON.parse + DOM 差分）は実機 DevTools Performance の
`attachAccessibilityMirror` tick で裏取りする（残タスク）。
