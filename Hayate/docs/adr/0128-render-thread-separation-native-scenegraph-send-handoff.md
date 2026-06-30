# render-thread 分離：native は UI/Raster 二スレッド（SceneGraph を `Send` 境界）、Web は OffscreenCanvas＋単一 Worker（非対称を許容）

**Status: proposed (draft)**

**Date: 2026-06-30**

## Context

ADR-0003 はコア（Scene Graph 更新・Layout・Render Command 生成・GPU 送信）を単一スレッドで設計し、その理由として WASM の SharedArrayBuffer/COOP-COEP 非保証と wgpu の `!Send` を挙げつつ、**「レンダースレッド分離（Flutter/Impeller 方式）は将来の ADR として予約する。API が安定した後、`Send + Sync` 境界を設計する」**と明記していた。本 ADR がその予約を解消する。

動機は ADR-0125（compositing layer incremental rendering）でラスタを激減させてもなお、native では「重い初回 paint・巨大レイヤの再 raster が入力スレッドを止めない」価値が恒久的に残ること。Blink（compositor 専用スレッド＋GPU プロセス）も Flutter（UI スレッド／Raster スレッド分離）も、レイヤキャッシュを持ったうえで二スレッドである。本命は native モバイル（Android/iOS）であり、スレッドモデルは **native を設計ドライバ**にする。Chrome を軸にしない。

## Decision

**正準スレッドモデルは native の UI/Raster 二スレッド分離。スレッド境界は SceneGraph の受け渡し。Web はこれを OffscreenCanvas＋単一 Worker で近似する別形とし、両者の非対称を正式に許容する。**

### native（正準形）

- コア（tree / layout / scene_build / lowering）は ADR-0003 どおり **単一スレッドのまま「UI スレッド」**に留める。コアをマルチスレッド化しない。
- **ADR-0125 のレイヤキャッシュ＋専用 compositor（Vello raster + wgpu compositor）だけを `Send` 可能な raster seam の裏に置き、専用 Raster スレッドへ**移す。Flutter と同型。
- **スレッド境界＝SceneGraph（＋`layer_dirty`）の受け渡し**。UI スレッドが produce、Raster スレッドが consume する。`Send + Sync` 境界はこのハンドオフに引く。
- Android は `Choreographer`、iOS は `CADisplayLink` 駆動。ADR-0117 の `tick`/`request_redraw` 契約は維持。
- **native 分離はコミット済みフェーズ**（ADR-0125 Phase 5）。③ の成否と独立に native では恒久価値がある。

### Web（近似形・非軸）

- Web で native の UI/Raster 内部二分割を再現するには SharedArrayBuffer（COOP/COEP）が必須で、ADR-0003 が却下した非実践パス。**Web ではこれを真似ない。**
- 代わりに **OffscreenCanvas＋単一 Worker にエンジン丸ごと**（WASM コア＋Vello raster＋compositor）を載せ、main スレッドは「DOM/pointer/IME を postMessage で Worker へ橋渡しする薄い shim」にする。COOP/COEP 不要で「main/DOM スレッドを空ける」という Web で本当に効く性質を得る。
- Web では **SceneGraph はスレッドを跨がない**（Worker 内に core も raster も同居）。よって `Send` 境界は native 専用で、Web は別機構。
- Tsubame(solid) reactivity も Worker 側へ移る。**IME(EditContext) は main 結合（ADR-0069）なので main↔Worker の IME ブリッジが Web 固有税**として必要。
- **Web Worker 化は計測ゲート**（「③ 後も main が詰まる」と出てから）。Web は軸でないため。

## Considered Options

- **Web/native を同型化（SharedArrayBuffer で Web も二スレッド）**: COOP/COEP が全環境で保証されず埋め込み/3rd party で壊れる、wgpu `!Send` を跨ぐ再設計が要る。ADR-0003 が却下済み。却下。
- **raster だけ Worker・ツリーは main**: SceneGraph を毎フレーム跨ぐコストが高い。却下。
- **④ 丸ごと計測コンティンジェンシー**（③ で足りれば一切やらない）: native の正準モデルを「やらないかも」に降格する。native 分離はコミットし、Web Worker のみ条件付きとする方針を採用。

## Consequences

- ADR-0003 の「将来予約」を解消し、`Send + Sync` 境界＝SceneGraph ハンドオフを確定する。コア単一スレッド原則は維持（破らない）。
- native では重い raster が入力/レイアウトを止めない。Web ではレンダリングが main/DOM を止めない（近似形）。
- Web/native のスレッド機構が非対称になる（native=実二スレッド＋共有メモリ、Web=単一 Worker でオフメイン）。これは「Chrome を軸にしない」設計の意図的帰結として許容する。
- ADR-0125 の cache/compositor は Phase 2 時点から **`Send` クリーンな seam の裏**に実装しておく（現スレッド実行でも seam を確保）ことが前提。

## 関係

- **supersedes** ADR-0003 の「render-thread 分離は将来 ADR」予約部分（コア単一スレッド原則自体は維持）。
- **depends on** ADR-0125（分離対象の cache/compositor seam）, ADR-0117（tick/request_redraw）。
- ADR-0087（Android first native target）, ADR-0114（iOS second native target）, ADR-0112（Android Tsubame JS / Hermes ブリッジ）, ADR-0118（desktop winit/vello）の native/desktop front が本スレッドモデルを実装する。
