# AccessKit 対応順序：ネイティブ優先、Web Canvas Mode は Safari EditContext API 対応後

AccessKit の統合を以下の順序で行う。

1. **ネイティブ（Windows / macOS / Linux）** — 最初に実装する。`accesskit` クレートだけで完結し、実装コストが最小。
2. **Web HTML Mode** — ネイティブ対応後。実 DOM に ARIA 属性を付ける形で対応する。
3. **Web Canvas Mode** — Safari が EditContext API を正式サポートしたタイミングで最優先で実装する。それまでは対応しない。

## 理由

Web Canvas Mode（WebGPU + EditContext API）は現時点で Chromium 系ブラウザのみで動作する。Hayate の主な強みは「Chrome 系ブラウザで UI 確認がそのままネイティブ品質で行える DX」であり、Canvas Mode のアクセシビリティ未対応は現段階でビジネスリスクにならない。

Safari が EditContext API を導入した場合、Canvas Mode が全主要ブラウザで動作するようになる。この時点で Web の UI フレームワーク勢力図が変わりうるため、その瞬間を Web Canvas Mode アクセシビリティ対応のトリガーとする。

Web Canvas Mode のアクセシビリティは `accesskit-web` クレートが必要であり、`<canvas>` の隣に不可視 ARIA DOM を動的生成する JS 連携が必要になる。この実装コストを Safari 対応前に払う理由がない。

## Consequences

- ネイティブ対応を先に設計・実装することで、`poll_accessibility()` WIT 関数・role enum・ElementId ↔ AccessKit NodeId マッピングの設計が固まる
- Safari EditContext API 対応は他の全タスクを中断して最優先で行う
- Web HTML Mode のアクセシビリティは ARIA 属性の付与のみで済むため、Canvas Mode より先に対応できる
