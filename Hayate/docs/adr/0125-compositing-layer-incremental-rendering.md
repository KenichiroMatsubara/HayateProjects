# compositing layer による damage ベース incremental rendering（raster/composite 分離）

**Status: proposed (draft) — Phase 2+（バックエンド半分・layer-present feature）は ADR-0135 により封印中。有効化禁止**

**Date: 2026-06-30**

## Context

中位モバイル GPU（例: Nothing Phone 3a / Adreno、Chrome）で描画がカクつき、ThinkPad / Mac mini M4 では滑らか、という性能差が報告された。原因はハードウェア依存だが、コード分析で構造的要因が特定できた：

- 現状の `render_scene`（`crates/platform/web/src/backend/vello.rs`, desktop / android も同型）は **SceneGraph 全体を毎フレーム Vello Scene に変換し、オフスクリーン target へ全面描画**してから present する。scene **lowering** は ADR-0086 で dirty-gated incremental 化済みだが、その先の **ラスタライズはサーフェス全面を毎フレーム再計算**している。
- コストは概ね `ピクセル数 × シーン複雑度` に比例する。モバイルは DPR≈2.5–3 でピクセル数が DPR² で膨らみ（ADR Tsubame-0007 で物理 px 描画・上限なし）、かつ GPU compute スループットがデスクトップの数分の一。掛け算でジャンクになる。

Blink（`cc` + Viz）も Flutter（Impeller）も、この問題を **DPR を下げずに** 解いている。共通戦略は「**raster は内容が変わったレイヤだけ・高コスト／composite は毎フレーム・低コスト**」の分離と、レイヤ/タイルの **texture キャッシュ**である（Blink: layer→256² tile→partial raster→partial swap、Flutter: RepaintBoundary→cached layer texture）。Hayate にはこの raster/composite 分離が無い。

本 ADR は「コストを画面全体ではなく **変化した分（damage）** に比例させる」ための compositing layer モデルを定義する。北極星は ADR-0102（Canvas の視覚基準は DOM）であり、レイヤ化は **作者に見せない最適化** とする。

## Decision

**SceneGraph に compositing layer の概念を導入し、レイヤ単位の retained texture キャッシュ＋専用 compositor で incremental に描く。**

### 1. レイヤ境界は自動・compositing-trigger 駆動（作者 API なし）

- どの `ElementAnchor`（ADR-0086）サブツリーがレイヤかは **compositing trigger** から自動判定する。初期トリガ集合は **{アクティブな transition/animation, `transform` group, scroll コンテナ}**。`opacity` は damage 的に静的なので初期は含めない。
- `will-change` 等の作者 opt-in primitive は **導入しない**（closed property vocabulary・ADR-0071 と DOM parity・ADR-0102 に反する）。自動で取りこぼす病的ケースが計測で出た時に明示 hatch を別 ADR で検討する。
- レイヤ id ＝境界要素の `ElementId`（ElementAnchor 同一性を再利用）。

### 2. レイヤツリーと layer-dirty は **コア**、cache＋compositor は **バックエンド**

- **コア（SceneGraph）**: `ElementAnchor` に compositing-layer フラグを持たせ、レイヤツリーを load する。レイヤの dirty 判定は **ADR-0099 の dirty routing から導出**した `layer_dirty` 集合として表現する（要素 dirty → それを内包する最近接レイヤを dirty）。プラットフォーム非依存。
- **バックエンド**: レイヤ単位のキャッシュ面（Vello = wgpu texture / tiny-skia = `Pixmap`）と合成を実装する。`render_scene` は「`layer_dirty` のレイヤだけ raster、残りはキャッシュ面を再利用」へ変わる。同一 SceneGraph を共有するため **tiny-skia(CPU) も同じレイヤ化の恩恵**を受ける（ADR-0048 のパイプライン不変）。

### 3. レイヤキャッシュは whole-layer texture（Flutter 流）

- レイヤ＝1 texture。スクロールはキャッシュ texture の平行移動で安く済ませ、再 raster は内容が変わった時のみ。Vello dispatch 数を抑える（モバイルは小 dispatch のオーバーヘッドが効く）。
- 全レイヤの Blink 流タイル化（256²）は **初手では行わない**。タイル化は scroll コンテナレイヤに限って後追いで足す（ADR-0126）。

### 4. 合成は専用 wgpu compositor（raster と別経路）

- キャッシュ texture を **transform / opacity 付き textured quad** として 1 render pass で合成する（Blink Viz / Flutter compositor 相当）。既存の `target_view` ＋ `TextureBlitter` を「単一 blit」から「quad 合成パス」へ拡張する自然な発展。
- **合成に Vello は使わない**（合成だけのフレームで Vello フルパイプラインを起動するのはモバイルで損）。Vello は「レイヤを塗る時だけ」起動する。
- compositor は **軸並行 clip のみ** を扱う。角丸 clip・border-radius は当面 **レイヤ内容に焼き込む**（Vello/tiny-skia 側で raster）。blur/backdrop 等の合成時エフェクトは別 ADR。

## ロールアウト（Phase 0→5）

本 ADR 群は段階導入し、各 Phase に native（Phone 3a 実機）計測ゲートを置く。

- **Phase 0** — ① on-demand 契約の遵守（ADR-0126）。アダプタ修正のみ。Gate: idle で 0 フレーム＋ベースライン計測。
- **Phase 1** — 本 ADR の **コア半分**（レイヤツリー＋`layer_dirty`）。バックエンド不変＝出力同一。Gate: golden-frame parity（ADR-0079）不変・dirty 単体テスト。
- **Phase 2** — 本 ADR の **バックエンド半分**（Vello レイヤ texture キャッシュ＋専用 compositor）＋ ⑤(a) pipeline warmup（ADR-0130）。cache/compositor は **`Send` クリーンな seam の裏**に置く（④ の布石）が実行は現スレッド。**Gate: Phone 3a がスクロール/transition で 60fps**。
- **Phase 3** — メモリ予算＋scroll overscan（ADR-0127）。Gate: 長リストが有界 VRAM で 60fps。
- **Phase 4** — ② 適応的レンダースケール劣化（ADR-0129）。熱/過負荷が計測で残れば。
- **Phase 5** — ④ render-thread 分離（ADR-0128）。native コミット・web Worker は計測ゲート。

## Considered Options

- **damage-rect で scissor するだけ**（レイヤキャッシュなし）: Vello の binning/coarse raster は scene encoding 全体を走るため、出力をクリップしても GPU compute はあまり減らない（節約は blit/present 帯域のみ）。B のゴールに届かず却下。
- **上流 `vello_hybrid` + sparse strips に全賭け**: incremental は上流の主戦場だが現状 beta 品質・API 流動・vendored fork 追従コスト大。レイヤキャッシュ層は特定レンダラ非依存で普遍なので、`vello_hybrid` 成熟時は「レイヤを塗る実装」として後から差し込める。今全賭けしない。
- **作者明示の RepaintBoundary**（Flutter 流）: DOM parity と closed vocabulary に反するため不採用（上記 Decision 1）。
- **Vello に合成もさせる**: 経路は 1 本になるが「composite は安い」原則に反する（Decision 4）。

## Consequences

- `SceneGraph` に compositing-layer フラグと `layer_dirty` 集合が増える。`render_scene` の契約が「全面描画」から「dirty レイヤ raster＋キャッシュ合成」へ変わる。
- Vello は「レイヤラスタライザ」に降格し、合成は専用 compositor が持つ。バックエンド差し替え（将来 `vello_hybrid`）が局所化する。
- tiny-skia(CPU) も同一レイヤ化で恩恵を受ける。
- 角丸 clip 焼き込みにより、transform でレイヤを拡大したとき角丸が甘くなる既知制限が出る（再 raster で解消）。許容し、必要なら ADR で SDF clip を後付けする。
- golden-frame parity（ADR-0079）は出力同値で守る安全網。

## 関係

- **extends** ADR-0086（retained incremental scene lowering / ElementAnchor）, ADR-0099（visual invalidation routing）。
- **uses** ADR-0048（共有 SceneGraph パイプライン・tiny-skia フォールバック）, ADR-0021（z-index = 子順序）, ADR-0020（transform = group node）。
- ADR-0126（on-demand 遵守）, ADR-0127（メモリ予算/scroll overscan）, ADR-0128（render-thread 分離）, ADR-0129（適応劣化）, ADR-0130（pipeline warmup）が本 ADR を取り巻く。
- 北極星 ADR-0102（Canvas 視覚基準は DOM）。
