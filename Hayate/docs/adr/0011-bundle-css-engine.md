---
status: superseded — Browser Extension ユースケースおよび DOM 互換を設計目標から除外。CSS 解釈は Hayate Element Layer が独自スタイルシステムとして内包する（HTML/CSS 完全互換を目指さない）。
---

# CSS エンジンを同封する

Browser Extension として既存ページの reflow コストを消すには、ブラウザの CSS 計算に依存してはならない。CSS エンジンを WASM に同梱し、ブラウザの rendering pipeline を完全に迂回する。HTML/CSS を自前で解析・カスケード・レイアウト計算し、Absolute Layout Tree を生成して Hayate Mutation に流す。

実装は段階的に改善する：

- **Phase 2**: Taffy（Flexbox/Grid）— CSS cascade なし、基本レイアウトのみ
- **Phase 3**: Servo/stylo — CSS cascade + フルレイアウト
- **Phase 5**: Blink 互換 — 完全 HTML/CSS 互換

## Considered Options

- **ブラウザ計算結果の抽出**（ADR-0010 で検討）: `getBoundingClientRect()` + `getComputedStyle()` で抽出すれば CSS エンジン不要と考えたが、これらの API はブラウザが reflow 後にしか呼べず reflow コストは消えない。ADR-0010 として記録し破棄。
- **CSS エンジン同封**: Extension サイズは 100 MB 超になりうるが、reflow を完全に消すにはこれしかない。サイズはフェーズを追うごとに最適化する。

## Consequences

- Extension の初期サイズは大きくなる。Taffy フェーズでも CSS cascade がないため完全互換にはならない。
- Taffy はこれまで「アプリ向けオプション Layout Engine」だったが、CSS エンジン段階的改善の第一歩でもある。
- Native Runtime でも同じ CSS エンジンを使えるため、ブラウザ非依存で一貫した描画が得られる。
