# z-index は同一 parent 内の子ソートで実現する。Stacking context は持たない

## Context

要素の描画順序（前後関係）を制御する z-index の実現方法として二つの候補があった。

1. **子ソート方式**: `StyleProp::ZIndex(i32)` を追加し、`scene_build::walk()` が同一 parent 内の子を z-index 値の昇順にソートして走査する（painter's algorithm）
2. **Stacking Layer 方式**: `NodeKind::Layer` を追加し、高 z-index 要素を root 直下の独立した Layer に昇格させる。CSS の stacking context に相当し、任意の要素を他の全要素より前景に出せる

## Decision

**子ソート方式を採用する。**

Hayate は CSS カスケードと stacking context を持たない設計である（Tier 4 有害として除外）。z-index に CSS と同等のセマンティクスを与えると stacking context の形成ルール（`opacity < 1`、`transform`、`position: fixed` 等による暗黙の stacking context 生成）を全て実装する必要が生じ、Hayate が捨てた CSS 仕様の複雑さが再導入される。

同一 parent 内での描画順序制御という用途に限定すれば、子ソート方式で十分である。モーダル・tooltip のように「全要素の上に表示したい」ケースは、アプリが root 直下に要素を配置することで解決する（React Portal に相当）。

## Considered Options

- **Stacking Layer 方式（却下）**: CSS の stacking context に近い表現力を得られるが、暗黙の stacking context 形成条件、z-index の継承と分離、`position: fixed` との相互作用等を実装する必要がある。ADR-0011 で除外した CSS 仕様の複雑さを部分的に再導入することになる
- **子ソート方式（採用）**: `scene_build::walk()` がルートに向かう前に各 parent の `children` リストを `ZIndex` 値でソートする。SceneGraph・NodeKind の変更不要。「全体の最前面に出したい」需要はアプリ設計（root 配置）で吸収する

## Consequences

- `StyleProp::ZIndex(i32)` を `style.rs` に追加する
- `scene_build::walk()` が子リストを `ZIndex` 昇順にソートしてから再帰する
- 「親の兄弟より前景に出る」挙動（CSS の `z-index: 9999` 的用途）は、アプリが root 直下に要素を配置するパターンで対処する
- HTML Mode では要素に `z-index` CSS プロパティを直接設定する（DOM の自然な描画順序と一致）
- Stacking context は存在しない。`transform` や `opacity` が暗黙の stacking context を作ることはない
