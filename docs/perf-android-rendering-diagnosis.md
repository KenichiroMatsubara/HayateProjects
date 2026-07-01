# Android 描画パフォーマンス診断（vello モードが重い / tiny-skia が極端に遅い）

2026-07-01。ブランチ `claude/android-vello-perf-cb3wwm`。

症状: Android で描画すると vello モードなのに非常に重い。tiny-skia（CPU）モードはさらに桁違いに遅い。

## 計測方法（フィードバックループ）

実機は使わず、フレームごとに走る仕事をホスト（x86_64, `--release`）で分解計測する
perf プローブを追加した。シーンは実アプリ相当の共有 fixture
`hayate_demo_fixtures::tasks_tree`（980x1060、446 ノード = rect 161 / text run 159 /
グリフ 516 / anchor 126）。

```
HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-tiny-skia --test perf_probe -- --nocapture
HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-vello     --test perf_probe -- --nocapture
```

GPU 側は llvmpipe（Mesa ソフトウェア Vulkan）で vello フルパイプラインを駆動した。
絶対値は実機 GPU を代表しないが、フレーム毎の異常（バッファ churn・キャッシュ肥大・
経時劣化）の検出には使える。

## 計測結果（p50）

| 項目 | scale 1 (980x1060) | scale 2 | scale 3 (≒ DPR 3 実機相当) |
|---|---|---|---|
| `tree.render`（アイドル・変更なし） | 0.04 ms | — | — |
| `tree.render`（コールド初回） | 5.5 ms | — | — |
| vello `Scene` フルエンコード（CPU） | 0.16 ms | — | 0.16 ms |
| tiny-skia `render_scene` 全面ラスタ | **37.5 ms** | **124 ms** | **265 ms** |
| 〃 テキスト無しバリアント | 29.6 ms | — | — |
| blit 前処理（`to_vec` + unpremultiply） | 6.5 ms | 27 ms | 75 ms |
| vello フル render（llvmpipe, 参考値） | 36 ms | — | 256 ms |

補助データ:

- fixture の rect 塗り面積合計 = **ビューポート 11.1 枚分**（オーバードロー係数）。
  scale 3 では約 104 MPx/フレームの CPU 塗り。265 ms 実測と定量的に一致する。
- tiny-skia の `simd` feature を有効化しても差なし（±2%）→ ブレンドループではなく
  フィルレート（オーバードロー）律速。
- 同一 renderer で 40 フレーム連続 vello render → 前半/後半で差なし。フレーム毎の
  リーク・肥大なし。

## 結論（原因のランキング）

### 1. 毎フレーム「全画面・物理解像度・フルシーン再ラスタ」する構造（両モード共通の主因）

コアの差分追跡は無実（アイドル 0.04 ms）。しかし present 経路は毎フレーム

- SceneGraph 全体 → レンダラシーンへフルエンコードし、
- ビューポート全域を物理解像度でラスタし直す。

1 px の変化でも全画面を塗り直す。ADR-0125/0127/0128/0130 のレイヤキャッシュ・
composite-only フレーム・Raster スレッド分離は **ホストテスト済みだが、どの
プラットフォームバックエンドにも配線されていない**（`hayate-scene-renderer-compositor`
に依存する platform crate はゼロ。`rg "compositor" crates/platform --include=*.toml`
が空）。よって「合成は安い・毎フレーム / raster は dirty レイヤだけ」は絵に描いた
状態で、実機は常にフルパイプラインを起動している。

vello（GPU）モードが Android で重いのはこれが直撃する。モバイル GPU（Adreno/Mali）
は vello のコンピュートパイプラインが不得手な上、1080x2400 級の全画面を毎フレーム
再ラスタするため、スクロール・タイピング・トランジション中は毎フレーム数十 ms の
GPU 時間を要求し、ジャンク＋発熱になる。

さらに Android は `ANDROID_CONTENT_SCALE = 1.0`（`surface_lifecycle.rs`）で
レイアウト＝物理 px。DPI スケーリングが無いのでラスタ面積は常に最大値になる。

実機での裏取り（残タスク）: `adb shell dumpsys gfxinfo` ないし logcat に
`render_frame` 前後のタイムスタンプを一時ログして、フレーム時間が「変更の大きさに
依存せずほぼ一定」であることを確認する。これが確認できれば本項が確定する。

### 2. tiny-skia モードは上記に加えて CPU ラスタの構造コストが乗る（「とんでもなく遅い」の内訳）

ホストネイティブですら全面ラスタ 37〜265 ms/フレーム。実機（CPU 3〜5 倍遅い＋
WASM 2 倍前後）では **1 フレーム数秒級**になり体感と一致する。内訳:

- **オーバードロー 11.1 枚分の AA rect 塗り**（約 8 割）。dirty-rect も遮蔽カリングも
  不透明矩形の fast path も無く、全 rect を毎フレーム AA 付きで塗る。
- **グリフキャッシュ皆無**（約 2 割）。`painter.rs` は毎フレーム・毎グリフ、skrifa で
  アウトラインを取り出し `PathBuilder` でパス化して AA 塗りする（516 グリフ/フレーム）。
  ラスタ済みグリフのアトラスキャッシュが無い。
- **blit 前に全バッファ `to_vec()` コピー＋unpremultiply**（scale 3 で 75 ms/フレーム）
  → `putImageData`。
- 補足: `push_clip_rect` は全画面 `Mask` の生成（ネスト時は親 Mask の clone）を伴う。
  この fixture ではクリップ 0 だが、scroll-view を含む画面では 1 クリップ＝全画面
  マスク 1 枚分の追加コストになる。

### 3. 潜在バグ: `to_vello_image` が毎フレーム新しい `Blob` を作る

`crates/scene-renderers/vello/src/lib.rs` の `to_vello_image` は draw のたびに
`Blob::new(Arc::new(image.data.to_vec()))` する。vello の画像アトラスは **Blob id
キー**（`vello_encoding/src/image_cache.rs`）なので、毎フレーム id が変わり
キャッシュ永久ミス＝**画像を毎フレーム CPU コピー＋GPU 再アップロード**する。
今回の fixture（画像 0 枚）では露出しないが、画像を含む画面では vello モードの
重さに直結する。`RenderImage` 側で `Blob` を保持して id を安定させるべき。

### 4. 副次: present 経路の追加全画面パスとフレームペーシング

- vello 出力を Rgba8 storage テクスチャに描いてから `TextureBlitter` でサーフェスへ
  全画面コピーする 2 パス構成（Android/web 共通）。モバイルでは帯域コストが無視
  できない。
- Android ループは毎イテレーション先頭で `poll_events(Some(16ms))` ブロック後に
  pump/present するため、継続フレーム（慣性スクロール等）は poll のタイムアウトが
  実質のペーサーになる。60fps 上限の粗いペーシングで、主因ではないが計測時の
  ノイズ源。

### 棄却した仮説

- コア差分追跡の退行（アイドル 0.04 ms で棄却）
- vello シーンエンコードの CPU コスト（0.16 ms で棄却）
- vello レンダラのフレーム毎肥大・リーク（40 フレーム安定で棄却）
- tiny-skia `simd` feature 無効が主因（有効化しても差なしで棄却。ただし NEON/wasm
  simd128 向けに有効化自体は無害なので、fill 律速を解消した後に再評価の価値あり）
- Rust 側のビルドプロファイル（Gradle は `profile = "release"` で正しい）

## 対処の優先順位（提案）

1. **ADR-0125/0127/0128 の配線**: `LayerCache` + composite-only `FramePlan` +
   （native は）`RasterThread` を Android / web バックエンドに実際に接続する。
   clean フレームで vello を起動しない・dirty レイヤだけ再ラスタする、が最大の効果。
2. **Android の DPI スケーリング導入**（`ANDROID_CONTENT_SCALE` 実値化）と、
   それに伴う論理解像度でのレイアウト。ラスタ面積の削減はそのままフレーム時間に効く。
3. **`to_vello_image` の Blob 安定化**（画像毎フレーム再アップロードの解消）。
4. tiny-skia モード: グリフのラスタキャッシュ（アトラス）、不透明 rect の AA なし
   fast path、`premultiplied_to_straight` のインプレース化（`to_vec` 排除）。
   ただし 1. のレイヤ化が入れば tiny-skia モードも「dirty レイヤだけ塗る」恩恵を
   受けるため、先に 1. をやるのが筋が良い。

## 再計測

perf プローブは env ゲート付きで両レンダラのテストに常設した
（`crates/scene-renderers/{tiny-skia,vello}/tests/perf_probe.rs`）。上記対処の前後で
同コマンドを流せば、レイヤ化の効果（clean フレームの raster ゼロ化）と fill 律速の
解消をホストで確認できる。
