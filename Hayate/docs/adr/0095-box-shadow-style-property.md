---
status: accepted
---

# box-shadow を Hayate CSS の visual プロパティとして追加する

POP 系 UI（影でカードを浮かせる・ハードシャドウ・フォーカスリング・inset）を Hayate CSS だけで表現できるようにするため、`box-shadow` を style プロパティとして契約に追加する。CSS フルパリティ（カンマ区切りの複数影）を採用し、各影は `offset-x` / `offset-y` / `blur` / `spread` / `color`（alpha 込み）/ `inset` フラグを持つ。意味論パリティのため DOM Renderer と Canvas Renderer（Vello）の両方が同一描画を行う。

## Considered Options

- **単一影・固定パラメータ** — wire が固定幅 struct で最も単純だが、gomi デモが実際に使う複数影・フォーカスリング（spread のみ）・inset を完全には再現できない。
- **最小（offset + blur + color のみ）** — spread と inset を落とすため、リング表現と内側影が不可能。
- **CSS フルパリティ（採用）** — 可変長の shadow リスト。既存の `dimensionList` wireKind に倣い、新 wireKind `shadowList` として `[count, {offsetX:f32, offsetY:f32, blur:f32, spread:f32, color:RGBA, inset:0|1} × count]` でエンコードする。DOM 側は `domFormat: shadow-list` で CSS 文字列を生成、Canvas 側は Vello で count 回描画（blur は gaussian、inset は内側クリップ、spread は外形ジオメトリ拡縮）。

## Consequences

- `box-shadow` を Transition の連続値補間対象に含める（ADR-0093 の effective-style seam モデルに乗る）。補間は CSS 準拠で、変化前後の shadow リスト長が等しく各 `inset` が一致するときのみ位置ごとに補間し、不一致は離散（即時 target 採用）。
- Canvas（Vello）側の複数影 + blur + inset 描画はコストがあるため、blur は許容範囲のガウス近似でよい。
- wire 形式（`shadowList`）は契約として一度公開すると変更しにくい。エンコード順とフィールド集合はこの ADR を正本とする。
- `proto/spec/style_tags.json` への追加 → `Tsubame/proto/generator` 経由で生成物を更新、CSS Gallery の Roadmap から live サンプルへ昇格。
