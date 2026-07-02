# Android Chrome の rubber バウンドが vello モードでも重い理由

2026-07-02。ブランチ `claude/android-chrome-rubber-bounce-wq0jv0`。
[perf-android-rendering-diagnosis.md](perf-android-rendering-diagnosis.md)（native Android、#637）の続編。

症状: Android Chrome（web アダプタ Canvas Mode）で、オーバースクロールの
rubber バウンド（スプリングバック）が vello（WebGPU）モードなのにカクつく。

## 計測方法（フィードバックループ）

実バウンス経路（`start_scroll_momentum` → `render` 内 `advance_scroll_motion` →
ばね積分 → scroll group アフィン再 lowering → present）をホストで 1 フレームずつ
駆動する perf プローブを常設した。シーンは Android 実機相当の縦リスト
（390x844 論理 px・80 行・scene 565 ノード）。GPU は lavapipe（Mesa ソフトウェア
Vulkan）で vello フルパイプラインを駆動（絶対値は実機 GPU を代表しないが、
「フレームコストが変更の大きさに依存するか」の判定には使える）。

```
HAYATE_PERF_PROBE=1 cargo test --release -p hayate-scene-renderer-vello \
  --test perf_probe_rubber_bounce -- --nocapture
```

env ゲートなしでも `rubber_bounce_mechanics` が repro の力学（バウンスが複数フレーム
アニメーションして端へ収束する・バウンス中の再 lowering が scroll-view 1 要素に
閉じる）を常時回帰固定する。

## 計測結果（p50, x86_64 `--release`）

| 項目 | 値 |
|---|---|
| スプリングバック継続フレーム数（overshoot 120px） | **23 フレーム（~383ms、毎フレーム dirty）** |
| バウンス中 `tree.render`（core 差分 lowering） | **0.135 ms/フレーム** |
| バウンス中の再 lowering 要素数 | **1（scroll-view のみ。SelfOnly reach）** |
| vello `Scene` フルエンコード（CPU） | 0.18 ms |
| GPU フル render 390x844（scale 1） | コールドフル 20.2 ms / **バウンス 1 フレーム 18.0 ms（比 0.89）** |
| GPU フル render 1170x2532（≒ DPR 3 実機） | コールドフル 48.1 ms / **バウンス 1 フレーム 54.0 ms（比 1.12）** |

読み方: バウンス 1 フレームで変わるのは scroll group の translate（アフィン 1 本）
だけなのに、present コストはコールドフル フレームと同じ（比 ~1.0）。つまり
**present は変更の大きさに依存しない = 毎フレーム全画面フルパイプライン**。

## 結論（原因のランキング）

### 1. present 経路に dirty ゲートが無く、バウンス毎フレームが「全画面・物理解像度・フルシーン」の vello render になる（主因）

core の差分追跡は無実（0.135 ms・再 lowering 1 要素）。しかし web の vello
バックエンド（`crates/platform/web/src/backend/vello.rs`）の `render_scene` は毎回

1. 新規 `vello::Scene` に SceneGraph 全体をフルエンコードし、
2. `render_to_texture` で vello の全段コンピュートパイプラインを
   **物理解像度**（canvas バッファ = CSS px × devicePixelRatio）で回し、
3. `TextureBlitter` でサーフェスへ全画面コピー（2 パス目）する。

ADR-0125/0127/0128 のレイヤキャッシュ・composite-only フレームは native 同様
**web バックエンドにも配線されていない**（#637 の結論は web にもそのまま当てはまる）。

rubber バウンドはこれの最悪ケースになる:

- スプリングバックは **~20〜40 フレーム連続のアニメーション**で、収束するまで
  `has_pending_visual_work()` が true → rAF 毎に必ずフル present が走る
  （`Tsubame/packages/renderer-hayate/src/hayate-renderer.ts` の継続判定）。
- 1 フレームでも 16.7ms を超えれば即ジャンク。指が離れた後の自走アニメーション
  なので、タッチ追従のごまかしが効かず、カクつきが最も目立つ。

### 2. Android Chrome は DPR ≈ 2.6〜3.5 → ラスタ面積が最大クラス

web アダプタは devicePixelRatio を正しく反映する（`resize_observer::canvas_resize_metrics`）
ため、1080p 級端末で canvas バッファは ~1170x2532 以上。計測でも scale 3 は
scale 1 の **~2.7 倍**のフレームコスト。native Android（`ANDROID_CONTENT_SCALE=1.0`）
とは逆に「正しく高解像度」なぶん、フルシーン present の面積コストが直撃する。

### 3. モバイル GPU × WebGPU は vello のコンピュートパイプラインが不得手

Adreno/Mali はストレージバッファ中心のコンピュートが遅く、さらに Chrome の
Dawn → Vulkan 変換を挟む。デスクトップなら「フルシーンでも 1〜2ms」で隠れる
構造問題が、モバイル WebGPU では 16.7ms 予算を普通に食い破る。加えて
Rgba8 ストレージテクスチャ → サーフェスの全画面 blit（2 パス構成）の帯域も乗る。

### 副次（今回は非主因と確認）

- vello `Scene` フルエンコード: 0.18 ms（WASM でも数 ms 級）。主因ではないが
  composite-only フレームが入れば丸ごと消える仕事。
- バウンス毎フレームの `Event::Scroll` 発火 → JS delivery、
  `element_scroll_max_offset` の subtree 走査 ×2/フレーム: いずれも µs 級。

### 棄却した仮説

- core 差分追跡の退行 / バウンス中に subtree 全再 lowering
  （walk_count == 1 で棄却。`SelfOnly` reach は子孫へ降りない）
- バウンス中の毎フレーム再レイアウト（scroll offset は visual-dirty のみで
  layout dirty を立てない）
- ばね物理そのものの計算コスト（`tree.render` 0.135 ms に含まれて無視できる）
- Android stretch profile（ADR-0131）固有のコスト — profile は scene lowering の
  アフィン 1 箇所の差だけで、iOS translate と同コスト構造

## 「vello モードなのに」への答え

vello かどうかは問題の位置ではない。**フレーム構造**（毎フレーム全画面・物理解像度・
フルシーン再ラスタ）が原因で、vello はそのコストを CPU から GPU に移しただけ。
モバイル GPU + WebGPU + DPR 3 では 1 フレームのフルパイプラインが予算を超え、
rubber バウンドは「毎フレーム dirty が ~0.4 秒続く」その最悪ケースとして表面化する。

## 対処の優先順位（#637 と同一方向）

1. **ADR-0125/0127/0128 の web バックエンドへの配線**: バウンス中に変わるのは
   scroll group のアフィンだけなので、レイヤ合成なら composite-only フレームに
   なり得る（raster ゼロ・合成 1 回）。効果は最大。
2. **canvas バッファ解像度の上限（DPR キャップ）**: 例えばモバイルで
   `min(dpr, 2.0)` に制限するだけでラスタ面積が ~50% 減る。1. までの暫定緩和策
   としても有効。
3. 実機での裏取り: Chrome DevTools の Performance トレース（Android リモート
   デバッグ）で、バウンス中のフレームが GPU 側で律速し、フレーム時間が変更の
   大きさに依存しないことを確認する。

## 再計測

プローブは env ゲート付きで常設（`crates/scene-renderers/vello/tests/perf_probe_rubber_bounce.rs`）。
1. の配線後に同コマンドを流し、「バウンス 1 フレーム vs コールドフル」比が
~1.0 から大きく下がる（composite-only 化）ことをホストで確認できる。
