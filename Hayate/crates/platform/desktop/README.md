# `platform/desktop/` — Desktop Family Adapter（枠のみ・ADR-0117）

Desktop family（macos / windows / linux）の **Family Adapter の枠**。現時点では **leaf が 0**
のため**ディレクトリ（枠）だけ**を置き、crate 化も capability trait の定義もしない。

grouping doctrine の正本は [`../README.md`](../README.md)。本ファイルはその枠マーカーである。

## なぜ leaf 0 で前払いするのか

ADR-0068 の「前払い（投機ではない seam）」条件を満たすため：

- **variation が確定済み**: Flutter plugin / RN TurboModule の prior art で、desktop capability
  の振れ幅（audio / clipboard / notification / file picker 等）は既知のカタログとして確定している。
- **確定ターゲット**: ADR-0012 で macOS / Windows / Linux は仮説ではなく確定した本体ターゲット。

枠と grouping doctrine を今引くことで、最初の desktop leaf 着手時に「どの段へ置くか」を毎回
再発明せずに済む。受容するリスクは、leaf 0 の段階で doctrine を引くため最初の leaf 着手時に
taxonomy 調整がありうること（ADR-0117 Consequences）。

## 枠の規律（今やらないこと）

- **capability trait を先置きしない。** 契約（trait）の正本は常に **Core**（`ImeBridge` /
  `Surface` / `FontFetcher` と同型・ADR-0068/0069）。最初の desktop capability の trait は、その
  capability を実装する**最初の desktop leaf 着手時に Core へ追加する**（空 trait を先置きしない）。
- **空 facade を作らない。** desktop は leaf 0 なので、`mobile` のような cfg(target_os) facade
  （crate）はまだ作らない。最初の 2 leaf が family 統一 capability を持って初めて facade を切る。
- **機構は借りない。** Flutter channel / RN bridge の**機構（ランタイム dispatch）は借りない**。
  借りるのは taxonomy（どの機能がどの段に属するかのカタログ）だけ。
