# Android stretch overscroll は一様スケール近似で core scene lowering 一箇所に閉じて実装する

status: accepted
refines: ADR-0113

## Context

ADR-0113 は「スクロール物理は Hayate Core が Scroll Physics Profile として所有する」と決め、iOS 風（指数減衰＋sigmoid rubber-band）を実装し、Android 風（OverScroller の spline fling＋Material stretch overscroll）は将来と据え置いた。

Android Chrome の「画面が伸びる」overscroll を実機に寄せるにあたり、以下を確定する必要があった。

1. **stretch の忠実度と実装コストのトレードオフ**: Android の EdgeEffect stretch は本来、引いた端に向かって非線形にフォールオフする歪み（`RenderEffect` のシェーダ）である。これを厳密に再現すると、レンダラごとにシェーダ／歪みメッシュを持ち込むことになり、tiny-skia(CPU) を含む全ターゲットでのパリティが崩れる。
2. **物理の二重持ち回避**: fling 減衰・spring back・rubber-band 変位は iOS profile 実装（`scroll.rs`）で既にテスト済みの純粋関数がある。Android のためにこれらを再実装すると、感触差でない部分まで枝分かれしてドリフトを生む。
3. **プロファイル差の局所化**: profile が物理・保存 offset・イベント・スクロールバーの各所に散ると、iOS の緑テストを壊さずに Android を足すのが難しくなる。

## Decision

**Android stretch overscroll を「一様スケール近似」で実装し、プロファイル差を core scene lowering の scroll group アフィン合成 *一箇所* に閉じる。**

- **一様スケール近似**: 越境変位に応じた単一のスケール係数 `scale = 1 + clamp(|overscroll_displacement| / dimension, 0, 1) * STRETCH_MAX` を、限界に達した端をビューポート境界にピン留めしたまま当該軸へ掛ける（純粋関数 `overscroll_stretch_scale`）。非線形フォールオフ歪みは近似せず、コンテンツを内側へ一様に伸ばすだけにとどめる。`STRETCH_MAX` は placeholder **0.15**（実機校正待ち）で、`physics` 定数ブロックの名前付き定数＋ `ScrollPhysicsTuning` の tunable フィールドとし、`tuning.json` で再ビルドなしに上書きできる（マジックナンバー禁止）。
- **scene lowering 一箇所に閉じる**: 伸びモードのとき scroll group の translate を `clamp(offset, 0, max)` に抑え、`overshoot = offset − clamped` を **ピン端アンカーの scale**（アンカー補正 translation を畳み込んだアフィン）へ変換して同じ group に合成する（`scroll_group_affine` / `stretch_axis`）。iOS モードは overshoot も translate に含める（scale 無し）——従来挙動を厳密維持。**物理・保存 `scroll_offset`・Scroll Gesture・`scroll` イベント・スクロールバー indicator は iOS と完全パリティ**で、既存の越境変位を read し替えて見せ方だけを変える（新しい物理状態を持たない）。
- **軸独立**: `max > 0` の軸だけ伸ばす（縦のみのページは横に伸びない）。両端で対称。
- **選択は dev `tuning.json` のみ**: web アダプタが `profile` を読み（`android`/`ios`/`auto`）、稼働 Scroll Physics Profile を core へ供給する。`Auto` の platform 識別（UA）による Android 自動解決は将来。`Auto` は iOS 据え置き（既定不変）。

## Considered Options

- **EdgeEffect の非線形 stretch をシェーダで厳密再現**: レンダラごとに歪みシェーダ／メッシュを持ち込み、tiny-skia(CPU) を含む全ターゲットのパリティが崩れる。忠実度は上がるが v1 のコストに見合わず却下（将来）。
- **Android profile で fling も OverScroller spline に差し替え**: spline 減衰は iOS の指数減衰と別アルゴリズムで、実装・テストの二重持ちになる。stretch の見た目より寄与が小さいため今回対象外（将来、ADR-0113 の宿題として残す）。
- **一様スケール近似＋ scene lowering 一箇所に閉じる（採用）**: 物理・イベント・保存 offset を iOS とパリティに保ったまま、overscroll の見せ方だけを純粋なアフィン合成で差し替えられる。golden/DrawOp 同値で iOS を固定でき、Android を加えても既存テストが緑のまま。

## Consequences

- ADR-0113 を refine。Android profile の **stretch 部分**が core に入る（一様スケール近似）。spline fling と非線形フォールオフ歪みは将来のまま。
- `ScrollPhysicsProfile::Android` variant と `uses_stretch_overscroll` 述語を追加。`default_tuning` は iOS とパリティ（物理は同じ）。既定 `Auto` → iOS は不変。
- `overscroll_stretch_scale`（core 純粋関数）と `STRETCH_MAX`（名前付き定数＋ tunable）を追加。scene lowering は `scroll_group_affine` / `stretch_axis` で overshoot を「端クランプ translate ＋ ピン端アンカー scale」に分割（ADR-0079 流の DrawOp 同値パリティで回帰固定）。
- web アダプタ `tuning.json` に top-level `profile`（`auto`/`ios`/`android`）と `scroll.stretch_max` を追加。未知の `profile` 値はパースエラーで拒否（タイポ表面化）。
- spec §7 SCR-01 を「Android profile: stretch 部分実装／spline 未」に更新、CONTEXT.md 用語集の Material stretch 記述を「一様スケール近似で部分実装」に微修正。
