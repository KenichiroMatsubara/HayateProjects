# Z-Order 乖離チェックは runtime 単独に一本化する（`DomStylePatch` 静的チェックは不採用）

> **ADR-0006 の一部を supersede** — 「静的コード」節（`DomStylePatch` の TypeScript `@deprecated` による IDE ファイル診断）を撤回する。「動的値」節（runtime `console.warn`）は変更なく有効。

_origin: architecture review（2026-07-03）_

## Context

ADR-0006 は、DOM Renderer が Canvas/Hayate と Z-Order セマンティクスで乖離しうるスタイル（`opacity` 等）に対して、二段構えの警告を採用すると決めた。

- **静的コード**: `DomStylePatch`（`packages/renderer-dom/src/dom-style-patch.ts`）が該当プロパティに TypeScript `@deprecated` を付与し、adapter が DOM ターゲット時にこの型へ分岐することで IDE 上に取り消し線を出す（任意・段階的）。
- **動的値**: `Z_ORDER_DIVERGENCE_PROPERTIES`（`packages/renderer-dom/src/z-order-divergence.ts`）が runtime の `setStyle` 呼び出しを見て `console.warn` する。

しかし `DomStylePatch` は `packages/renderer-dom/src/index.ts` から export されているだけで、リポジトリ全体を通じて実際に使う箇所が一つもなかった。`DomRenderer.setStyle` 自身も引数型を protocol の `StylePatch` のまま宣言しており、`DomStylePatch` を使っていない。

原因は実装の後回しではなく、設計上の不整合だった。Tsubame Adapter（`tsubame-react` / `tsubame-solid`）は **renderer 非依存**であることが前提で、1つの adapter が実行時に合成ルートの選択で DOM/Hayate どちらのターゲットにもなる（`packages/react/src` にも `packages/solid/src` にも `StylePatch` への参照は一つもない）。ADR-0006 が想定した「adapter が DOM ターゲット時に型分岐する」という利用箇所は、adapter を renderer 非依存に保つという Tsubame 自体の設計原則と両立しない。

## Decision

- `DomStylePatch`（`dom-style-patch.ts`）を削除し、`packages/renderer-dom/src/index.ts` からの export をやめる。
- Z-Order 乖離の警告は `Z_ORDER_DIVERGENCE_PROPERTIES` + `warnZOrderDivergence`（runtime `console.warn`、dev のみ、同一 element + property につきセッション 1 回）を唯一の seam とする。
- 新しい乖離プロパティ（例: `transform`）を追加する際は `Z_ORDER_DIVERGENCE_PROPERTIES` の1箇所だけを更新すればよい。

## Considered Options

- **`DomRenderer.setStyle` の引数型を `DomStylePatch` に変える（却下）** — `IRenderer` 越しに呼ぶ限り呼び出し元は `StylePatch` のまま渡すため、IDE の取り消し線はどのみち表示されない。DOM Renderer を adapter を介さず直接使うコード（例のみ）にしか届かず、実利用箇所に対して割に合わない。
- **adapter に DOM/Hayate の型分岐を導入する（却下）** — Tsubame の核となる設計（1 adapter が実行時にターゲットを選べる）を崩す。ADR-0004 の意図と衝突する。
- **runtime 警告のみに一本化する（採用）** — 唯一実際に機能している seam を正本とし、二重宣言と死んだ型を無くす。

## Consequences

- `packages/renderer-dom/src/dom-style-patch.ts` を削除。`index.ts` の export を1行削除。
- `Tsubame/CONTEXT.md` の「DOM Renderer」の記述を、runtime 警告のみの説明に更新済み。
- ADR-0006 の「静的コード」節はこの ADR に supersede されたものとして読む（ADR-0006 本文は決定当時の記録として残す）。
- 将来 IDE レベルの静的チェックを望むなら、adapter コードではなく実際に型が分かる呼び出し箇所（例: DOM Renderer を直接使うコード）を対象に、この ADR を踏まえて再設計する。
