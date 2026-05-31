# Element Layer uses deferred command queue; render() is the sole flush boundary

`element_*` 関数を呼び出してもコマンドは即時実行されず、Hayate内部の `Vec<Command>` にキューイングされるだけである。`render()` 呼び出し時に初めてキューが全件フラッシュされ、レイアウト計算・コミットが行われる。CanvasモードとHTMLモードで共通のセマンティクス。

## 背景

上層（Hayabusa等）がJSまたはWIT経由でHayateを呼び出す場合、`element_*` 関数1件ごとにJS→WASM境界またはWIT境界を越えるコストが発生する。1フレームで多数のコマンドを即時実行すると、境界コストが線形に積み上がる。

## 決定

`element_*` 関数はすべてコマンドをキューに積むだけとし、副作用（DOM操作・シーングラフ変更）を持たない。`render()` がキューをフラッシュする唯一の境界とする。別途 `flush()` エクスポートは設けない。

WITインターフェースのシグネチャは変更しない。セマンティクス変更のみ。

## Considered Options

- **即時実行（変更前）**: シンプルだが、JS/WIT境界コストがフレームあたりのコマンド数に比例して増大する。
- **別途 `flush()` エクスポートを追加**: `render()` が既に自然なフラッシュ境界として機能するため不要。

## Consequences

- `element-get-text` 等の読み取り関数は、直前の `render()` でコミットされた状態を返す。キュー内の未コミット値は反映されない。
- `render()` を駆動するのは引き続き上層（ADR-0018 維持）。HayateはrAFをインポートしない。
- Hayabusa側の変更は不要。
