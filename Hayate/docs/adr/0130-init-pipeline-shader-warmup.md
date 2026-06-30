# init 時パイプライン/シェーダ warmup（初回操作ジャンク撲滅）。native 永続キャッシュは後段

**Status: proposed (draft)**

**Date: 2026-06-30**

## Context

実行時のシェーダ/パイプラインコンパイルは初回描画フレームでスパイク（初回タップ/初回スクロールのカクつき）を生む。Flutter が Skia から Impeller へ移行した最大の動機がこの「first-run jank」撲滅だった。

Hayate では Vello の compute パイプラインは `VelloSceneRenderer::new()` で概ね前倒し生成済み。残る穴は ADR-0125 で新設する **専用 wgpu compositor のパイプライン variant（blend/surface format 別）** が初回合成時に遅延生成されてスパイクになる点。tiny-skia(CPU) はシェーダを持たないため対象外。

## Decision

**(a) init 時に全パイプライン variant を前倒し生成（warmup）する。これを v1 に入れる。(b) native の永続パイプラインキャッシュ（Vulkan/Metal をディスク保存）は後段の native 仕上げとする。**

- **(a) warmup（v1）**: エンジン初期化時に **Vello＋compositor の全パイプライン variant**（必要な surface format / blend 組み合わせ）を生成しておき、初回フレームで遅延生成が走らないようにする。低コストで初回操作スパイクを消す。
- **(b) 永続キャッシュ（後段）**: native で Vulkan/Metal のパイプラインキャッシュをディスクに永続化し、起動間でも cold start を速くする（Flutter/Impeller がやっている native 仕上げ）。体感寄与は二度目以降の起動に限られるため、native 最適化フェーズに回す。

## Considered Options

- **遅延生成のまま（warmup なし）**: 初回操作で確実にスパイク。却下。
- **(b) を v1 に含める**: 二度目以降の起動の話で初回体感への寄与が小さく、native 固有の工数が増える。後段で十分。

## Consequences

- 初回タップ/初回スクロールのカクつきが消える（低コスト・高費用対効果）。
- ロールアウトは ADR-0125 Phase 2（compositor 導入と同時。compositor が新パイプラインを持ち込むため）。
- (b) は将来の native 最適化 ADR で扱う。

## 関係

- ADR-0125（compositor パイプラインの導入元）, ADR-0128（native front）。
- ADR-0002（wgpu）, ADR-0006（Vello）。
