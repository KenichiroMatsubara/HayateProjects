# HTML/CSS レイアウトはブラウザの計算結果抽出に委譲する

Browser Extension として既存ページを NewDOM で描画する場合、CSS エンジンおよびレイアウトエンジンを WASM に同梱しない。代わりに `getBoundingClientRect()` + `getComputedStyle()` でブラウザが既に解決した Absolute Layout Tree を抽出し、NewDOM Mutation に変換する。

HTML/CSS 互換の実体は「CSS を実装すること」ではなく「ブラウザの計算結果を GPU 描画に変換すること」である。

## Considered Options

- **Servo/stylo を WASM に bundle する**: CSS カスケード・レイアウトを完全自律で計算できる。ただし stylo だけで 30〜50 MB、フルエンジンは 50〜100 MB 超となり、Extension として実用的でない。
- **ブラウザ計算結果の抽出**: CSS エンジン・レイアウトエンジン不要。WASM バイナリは 8〜15 MB 程度（wasm-opt 後）に収まる。`getBoundingClientRect()` が Absolute Layout Tree をそのまま返すため実装コストも最小。DOM Adapter の工数がほぼゼロになり、GPU 描画と低レイヤ互換 API の実装に集中できる。

## Consequences

- クロスオリジン iframe・`<canvas>` 内部・`<video>` フレームは抽出不可。これらは Extension Runtime のスコープ外とする。
- Extension が DOM を `display:none` で非表示にした後のイベント処理は、`nd_hit_test` で座標を解決し元 DOM 要素に `dispatchEvent` する構成を将来実装する（現時点では未実装）。
- ブラウザが CSS を計算していない環境（Native Runtime 等）では本アプローチは使えない。Native では別途レイアウト戦略が必要。
