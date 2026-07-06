# 命令的 2D 描画（draw）は記録型 display list として view の property で公開し、Raw Layer 公開棄却（ADR-0072）を維持する

**Status: accepted**

**Date: 2026-07-06**

## Context

Flutter `Canvas` 同等の表現力（曲線・ベジェ・任意パスの塗り/線、将来はグラデーション〜シェーダまで）を Hayate 上のアプリに提供したい（PRD #723）。描画バックエンド（Vello / tiny-skia）は任意パス描画を既にサポートするが、`NodeKind` / `ScenePainter` / proto 契約がそれを露出していない。

命令的な描画 API を出す素朴な形は Raw Layer（`SceneGraph` / `Node` 直接操作）の外部公開だが、それは ADR-0072 が「第2 wire 契約のコストに見合う需要がない」として正式棄却済み。一方で ADR-0072 が棄却したのは**第2の公開契約**であって、命令的な**書き味**そのものではない。Flutter 自身も `CustomPaint` は widget tree 上のノードであり、`paint(canvas)` の命令列は Picture（display list）として記録され retained scene に**値として**渡る——命令的なのは記録時だけで、公開境界は tree 1つのままである。

## Decision

**Flutter Picture モデルを採用する。** 命令的描画は「記録 → 値として単一 wire 契約に載る」形で提供し、公開サーフェスは Element Layer 1つのまま（ADR-0072 維持）。

- `view` に property `draw` を追加する。新 element-kind は作らない。`element_kinds` の `carriesDraw` タグで view 限定（carrier 文化 = `carriesTextLocal` の踏襲）。
- `draw` の値は painter（`{ paint(canvas, size), shouldRepaint?(old) }` または関数糖衣）。フレームワーク側が painter を呼んで display list を記録し、`apply_mutations` の新チャネル `draws: Float32Array`（`texts` と同格）で Hayate へ渡す。
- Hayate 内部では display list を保持する retained node を `NodeKind` に追加し、`ScenePainter` にパス描画メソッドを追加する（vello / tiny-skia / vello-cpu 全実装）。「眠っていた下層の復活」は **Raw Layer の内部語彙拡張**として実現する。
- 描画順は background → border → draw → children（将来 `draw-foreground` で `foregroundPainter` 対応）。座標はボーダーボックス左上原点・論理 px・DPR 不可視。クリップは既存 `overflow` に従う（既定 visible = Flutter `CustomPaint` 既定と一致）。hit-test は view の box 判定のまま（Flutter の `hitTest` 既定と一致。パス hit-test は将来 opt-in）。draw 変更は visual dirty のみで layout 不干渉。
- v1 スコープはパス幾何・単色 fill/stroke・座標操作/クリップ。グラデーション・blend・drawImage・テキスト・フィルタ/シェーダは**封印ではなく後回し**——encoding・enum 表・painter interface はこれらが契約破壊なしに生えることを合格条件とする。

## Considered Options

- **宣言的パス element（パス幾何を Hayate CSS / property として宣言）**: 命令的な書き味と Flutter `CustomPainter` 移植性を満たせず、size 依存の手続き的描画が書けない。却下。
- **ADR-0072 を supersede して Raw Layer を公開（第2契約）**: 「曲線を描きたい」需要は display list で満たせる。ADR-0072 の reopen 条件「需要が seam を本物にしてから」は未達。display list で足りない実需要（毎フレーム数万ノードの Infinite Canvas 等）が出た時点で reopen するのが筋。却下。
- **新 element-kind `canvas` を追加**: 「Canvas Mode」「Canvas 経路」と用語衝突が痛い。子を持たせるなら結局 view + 描画と同じ物になり kind を分けた意味が薄い。property 方式なら `CustomPaint(painter:, child:)` の合成モデルを最少語彙で写せる。却下。

## Consequences

- HTML Mode は v1 で draw 非対応（現状 Tsubame init から到達不能な dead path のため。生きた経路を先に揃える）。
- DOM Renderer は同じ painter を `<canvas>` 2D へ replay して意味論パリティを取る（Tsubame ADR-0014）。
- 記録 API の形と多言語生成は ADR-0142、painter の size 供給と 1 フレーム遅延は ADR-0143 が定める。

## 関係

- ADR-0072（Raw Layer 外部公開棄却）: **維持**。本 ADR は棄却済みの第2契約を復活させず、内部 lowering 語彙の拡張に留める。
- ADR-0086（Retained incremental lowering）: display list node はこの仕組みに乗る。
- ADR-0002（Hayate CSS 意味論のレンダラー非依存）/ 意味論パリティ: `draw` の描画意味論は Canvas 経路が正準、DOM Renderer が replay で一致させる。
