# Style Channel gate を renderer の手前の単一 seam で一度だけ適用する

**Status: accepted**

**Date: 2026-06-15**

## Context

ADR-0008 は意味論パリティ規則（text-local gate ほか）を spec 正本から生成し、
**「DOM の適用前・Canvas の encode 前の両方」**で同一の生成物を参照する、と定めた。
しかしその「両方で参照」は各 renderer が gate を**自前で呼ぶ**実装になっていた
（DOM は `style-declarations.ts`、Canvas は `canvas-renderer.ts` の `gate()`）。

結果として:

- gate の呼び出しが renderer ごとに重複し、両者の一致は parity テスト
  （`text-local-gate-parity` の DOM vs Canvas 比較）だけが担保していた。
- `IRenderer` を実装するのは DOM / Canvas の 2 実装のみで、parity テストは
  interface を越えて各 renderer の内部状態（CSSOM・wire batch）に手を伸ばしていた。
- 新しい renderer を足すたびに gate を**もう一度**実装する必要があり、抜ければ
  静かに乖離する。

## Decision

**gate を renderer の手前に置いた単一 seam で一度だけ適用する。**

- `withTextLocalGate(inner: IRenderer): IRenderer`（renderer-protocol）を追加する。
  これは `IRenderer` の decorator で、`createElement` から element の kind を学習し、
  style を伴う全 op（`setStyle` / `setPseudoStyle` / `setStyleVariant`）で
  `gateTextLocalPatch` を適用し、フィルタ済み patch を inner にそのまま転送する。
- mount（`renderTsubame`）が選択された renderer をこの seam で 1 回だけ包む。
  DOM / Canvas は gate を自前で持たず、**既にフィルタ済みの patch** を受け取る。
- `RecordingRenderer`（renderer-protocol）を追加する。呼び出しを記録する in-memory な
  `IRenderer` 実装で、seam を「2 つ目の adapter」で実体化し、テストが各 renderer の
  内部状態ではなく **interface 越し**に検証できるようにする。

ADR-0008 の「両 renderer で適用」を、その狙い（単一規則・構造による担保）を保ったまま
**「手前で一度だけ」**へ精緻化する。規則の生成（spec 正本）は ADR-0008 のまま。

## Consequences

- **意味論パリティが構造で担保される。** gate は 1 箇所だけなので、seam の背後の
  全 renderer は定義上同一のフィルタ済み patch を受け取る。parity は「test による
  担保」から「構造による担保」へ移る。
- **新 renderer は gate を持たなくてよい。** seam の背後に置くだけでパリティが保たれる。
- **テストが interface 越しになる。** `RecordingRenderer` 越しに記録された呼び出しを
  読むことで、CSSOM や wire batch といった renderer 内部に触れずに seam を検証できる。
- DOM の宣言 emitter（`declarationsFromStylePatch`）と Canvas の encode 経路は
  gate を持たなくなり、責務が「受け取った patch を忠実に出力する」に縮む。
- pseudo-state parity の corpus harness は seam 後の pipeline を表すため、emit 前に
  gate を適用する。

## 関係

- ADR-0008（意味論パリティ規則を spec 正本から生成）を精緻化する。規則の単一正本は
  ADR-0008 のまま、その**適用点**を単一 seam に集約する。
- ADR-0002（Renderer Protocol + DOM / Canvas の 3 層）の `IRenderer` 境界上に
  decorator として乗る。
