# HTML Mode の z-index は絶対座標レイヤー方式で実現する

## Context

ADR-0021 では z-index を「同一 parent 内の子ソート（painter's algorithm）」で実現することを決定した。
Canvas Mode では `scene_build::walk()` が子リストを z-index 昇順にソートして SceneGraph に積み、Vello が上から順に描画する。

HTML Mode では全要素が `position: absolute` + Taffy 計算済みの絶対座標（`left` / `top` / `width` / `height`）で配置される。この構造下では CSS の stacking context はコンテナ div 一つだけであり、各要素の CSS `z-index` 値は「同一 stacking context 内での描画順序」として機能する。これは Canvas Mode の painter's algorithm と等価である。

## Decision

**HTML Mode では CSS `z-index` プロパティを要素に直接設定することで描画順序を制御する。**

全要素が単一コンテナ直下の `position: absolute` であるため、CSS の stacking context ルール（`opacity < 1` や `transform` による暗黙のコンテキスト形成等）は発動しない。`z-index` は純粋に「絶対座標で積まれたレイヤーの順序」として機能し、Canvas Mode の子ソートと同じセマンティクスを持つ。

ADR-0021 で廃止した Stacking context 方式とは異なる。ここで言う「絶対座標レイヤー方式」は CSS stacking context の再現ではなく、絶対座標配置済みの要素群の描画順序を整数値で制御するだけのものである。

## Consequences

- HTML Mode での `z-index` は `StyleProp::ZIndex(i32)` の値をそのまま CSS `z-index` プロパティに設定する
- `transform` や `opacity` を持つ要素が暗黙の CSS stacking context を形成する問題は、全要素が単一の `position: relative` コンテナ直下に置かれることで実用上発生しない
- Canvas Mode と HTML Mode の z-index セマンティクスは同一 parent 内の描画順序制御に限定される点で一致する
