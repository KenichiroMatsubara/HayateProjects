# layer-present の白い矩形描画バグの根本原因は premultiplied/straight alpha の取り違え

**Status: accepted**

**Date: 2026-07-05**

## Context

ADR-0135 が実ブラウザで発見した描画バグ（#697）——スクロールでタスクリスト最下点付近に恒常的に出る白い矩形と、hover/tap で一瞬だけ出る白い矩形の2パターン——について、issue #699 で `/diagnosing-bugs` により根本原因を調査した。

実 Chrome（ヘッドフル、`--use-angle=gl`——このマシンのネイティブ Vulkan ドライバが不安定なため）で単発 wheel イベントによる決定的な再現ループを再構築し、`present_layers` に一時計装（各レイヤの抽出済み SceneGraph 全ノードのダンプ、`?soloLayer=<id>` による単独レイヤ描画の切り替え）を追加して調査した。root 単独描画は正しいクリーム色、panel（scroll-view）レイヤ単独描画も一見正しい暗い色を示すのに、両方を通常合成すると同じ座標が純白になるという、正しいアルファブレンド数式では説明できない矛盾が観測の決め手になった。

## Decision

根本原因は `hayate-scene-renderer-vello::layer_compositor::WgpuQuadCompositor` の合成 blend state が、レイヤ texture の中身を **premultiplied alpha だと誤って仮定**していたこと（`src_factor: One`）。実際には vello の `render_to_texture`（`VelloLayerRasterizer::rasterize` が呼ぶ）がレイヤ texture へ書き込む内容は **straight（非 premultiplied）alpha** であり、`One` ではなく `SrcAlpha` を使わない限り、半透明な内容（box-shadow のぼかし縁など alpha<1 のピクセル）の色成分が alpha で減衰されずにそのまま加算されてしまう。

不透明（alpha=1.0）な背景・ボーダー・テキストでは premultiplied と straight が同じ値になるため無症状——box-shadow のような半透明コンテンツを持つレイヤでのみ症状が出る。golden-pixel parity テスト（`layer_scene_parity.rs`・#691）が不透明色のみのシーンで固定されていたため、このバグを一切捕捉できなかった。

- **恒常パターン（スクロール）**: パネルの scroll-view レイヤが再 raster されるたびに、レイヤ内の box-shadow の半透明ピクセルがこの誤った blend で合成され、飽和して白潰れする。
- **一瞬パターン（hover/tap）**: hover/tap による pseudo-state 遷移も同じレイヤの再 raster・再合成を同一コードパスで発生させるため、同じ blend バグが同じように発火する（render-on-demand のフレームゲーティングにより数フレームで自然回復するので「一瞬」に見えるだけ）。両パターンは同一原因であることを、hover 側でも同じ計装（層の再 raster 発火の確認、修正前後の直接ピクセル比較——修正前は約19,500ピクセルが白飛びし修正後は完全に消える）で確認済み。

**修正**: `Hayate/crates/scene-renderers/vello/src/layer_compositor.rs`（`WgpuQuadCompositor::build_pipeline`）の `BlendMode::Alpha` の color blend `src_factor` を `One` → `SrcAlpha` へ変更。alpha チャネルの blend（`src_factor: One` のまま）は変更不要（alpha 合成式自体は premultiplied/straight で変わらない）。

修正後、非 layer-present（全面描画）版とのピクセル完全一致をスクロール量 0〜400 の全域で確認した。

**回帰テスト**: `Hayate/crates/scene-renderers/vello/tests/layer_present_parity.rs` に `layered_present_matches_full_raster_for_translucent_box_shadow_during_transition` を追加。半透明（非黒・alpha=0.3）の box-shadow を持つ要素を transition でレイヤ昇格させ、レイヤ分解 raster + quad 合成が全面 raster と画素一致することを固定する。修正前のコードで re-revert して実際に fail する（最大 25/255 差）ことを確認済み。**黒（0,0,0）のシャドウ色ではこのクラスのバグを検出できない**（straight/premultiplied どちらで解釈しても src 項が恒等的に 0 になり差が出ない）——非黒の彩度ある色が回帰テストには必須。

### 他バックエンドへの横展開確認

`hayate-scene-renderer-tiny-skia`・`hayate-scene-renderer-vello-cpu` はそれぞれ独自の `LayerCompositor` 実装を持つ（vello 版とは共有コードではない）。同じ手法で確認した結果:

- **tiny-skia**: 同種のバグ無し。`tiny_skia::Pixmap` はライブラリ自身の不変条件として常に premultiplied alpha を保持し、合成もライブラリの `draw_pixmap` に委譲しているため、Hayate 側が blend factor を独自に仮定する箇所が無い。回帰テスト（`tests/layer_compositor.rs::translucent_box_shadow_layer_cpu_composite_matches_full_raster`）で確認。
- **vello_cpu**: 同種のバグ無し。微小差（最大 4/255）はあるが shadow/box 境界に散在するパターンで、本バグの特徴（大きく一様な加算誤差）とは異なり、レイヤ分解時と全面 raster 時の AA/丸め順の既知のずれとみられる。回帰テストの許容差を 4 に調整して対処。

## Consequences

- 本修正は ADR-0135 が定める再開条件のうち「実ブラウザ描画バグの修正」を満たすが、**製品としての有効化再開には至らない**——再開条件はもう一方の「実機/実ブラウザで計測可能な性能上の実害が具体的に発生した時」も満たす必要があり、#697 が示唆した「性能上の優劣なし」は未解消のまま。ADR-0135 の `sealed` ステータス・Decision 本文は変更しない。
- 調査中に副次的に判明した2件は、本 ADR の対象範囲外として別 issue で追う: ADR-0127 の overscan サイジングが実際の present 経路に配線されていない件（#704）、ADR-0135 の feature gate が vello バックエンドのみを対象とし tiny-skia/vello_cpu の per-layer 合成は無条件で有効なままである件（#705）。

## 関係

- **amends** ADR-0135（layer-present 封印。Status に本 ADR への参照を追記）。
- **references** ADR-0125（compositing layer incremental rendering）, ADR-0127（layer cache memory budget / scroll overscan）。
- 動機となった調査: #699。
