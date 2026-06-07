# 擬似クラススタイルは Hayate CSS の一部として Render Layer が解決する

ADR-0019 は `:hover` 等のスタイル切替を Framework（Signal）の責務とした。Canvas Mode ではホバー時に JS→WASM の `setStyle` 往復と最深要素の hit 判定が重なり、視覚的にホバー領域内でも親のホバーが外れる等の不具合が起きた。DOM エンジンが擬似クラスをレンダラー側で解決するのと同型に、Hayate CSS に要素ローカルの `:hover` / `:active` / `:focus` を含め、ポインタ状態に応じた effective style を render 時に合成する。`hover-enter` 等のイベント delivery はアプリロジック用に残す。

## Considered Options

**event-driven のまま Tsubame 実装を直す案を却下。** ホバー判定は既に Hayate が担っており、スタイルだけ Framework に戻す二重モデルが残る。

**擬似クラスを Hayate CSS の文法に含めず別 prop にする案を却下。** `style` オブジェクト内の nest（`:hover` キー）として宣言する方が Hayate CSS の一本化に合う。

## Consequences

- ADR-0019 のスタイル責務分担を撤回（イベント通知は温存）
- `:hover` は CSS 互換：子孫上にポインタがあれば祖先もマッチ。`hit_test` の最深要素返却は click 等の別用途のまま
- セレクタ・カスケード・スタイルシートは引き続き Hayate CSS の対象外
