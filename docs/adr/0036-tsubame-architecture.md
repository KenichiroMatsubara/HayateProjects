# Tsubame は SolidJS 的 Signal + TSX、DOM/Canvas 二モード構成とする

JS/TS ユーザー向けフレームワーク Tsubame の設計として、Virtual DOM + React hooks ではなく SolidJS 的 fine-grained Signal（createSignal / createEffect / createMemo）と TSX 記法を採用する。コンポーネント関数は一度だけ実行され、Signal の変化のみが対象の mutation を発火する。

DOM Mode では Hayate を使わず JS バンドル + HTML シェルの CSR 成果物を直接利用する（JS→WASM 境界なし）。Canvas Mode では Signal の変化をフレーム単位で JS Array にバッチ化し、apply_mutations(batch) で Hayate に 1 回/frame 渡す。共通の element 型（view / text / image / button 等）をモード間で統一し、コンポーネントコードはモードを意識しない。

## Considered Options

- Virtual DOM + React hooks: React Native と真正面から競合し差別化できない。エコシステム・知名度で劣る
- Signal + React hooks ファサード（Preact Signals 方式）: 「似てるが違う」混乱を生む。中途半端
- SolidJS 記法採用: 「GPU レンダリング × SolidJS」は競合不在のカテゴリ。Hayate の retained + fine-grained mutation との統合効率も高い
