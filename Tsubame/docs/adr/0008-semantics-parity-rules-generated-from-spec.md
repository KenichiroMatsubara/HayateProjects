# Semantics Parity 規則を spec 正本から生成し、両 renderer で適用する

**Status: accepted**

**Date: 2026-06-15**

## Context

意味論パリティ（CONTEXT の「意味論パリティ」）は、Renderer Protocol の語彙が名前だけでなく意味論ごと契約であるという原則だが、その**規則そのものが実装ごとに散在**している。

- **text-local gate**：channel-1 text-local スタイルが Text-Local Carrier kind（`text` / `text_input`）にのみ届くという規則（Hayate `proto/spec` の `style_tags.inherit` ＋ `element_kinds.carriesTextLocal` が正本）は、現在 **DOM Renderer の style 適用経路（`style-declarations.ts` の `shouldApplyTextLocalPatch` gate）だけ**に実装されている。Canvas 経路（`encode-mutations` → Hayate）は gate を持たず、Hayate 内部 lowering の棄却に依存する。
- **pseudo band order**：擬似状態の正準優先順（`focus < hover < active`、`proto/spec/pseudo_states.json` が正本）は、protocol の `PSEUDO_STATE_PRIORITY` 定数と、DOM Renderer の `pseudoPriorityFromSelector()`（selector 文字列への正規表現）に**二重実装**されている。
- **prop-op の適用**：`coerceElementProperty` の結果を DOM 書き込み / Canvas enqueue に振り分ける解釈が、両 renderer の `setProperty` に別々に書かれている。

規則が実装ごとに再宣言され、DOM と Canvas が静かに乖離しうる。パリティが interface ではなく実装で担保されているため、mock / test double では検出できない。

## Decision

**spec 正本から生成し、両 renderer で同一の生成物を適用する。**ADR-0070（domCss 単一正本）・ADR-0055（encodeFrom）の「spec 正本 → 両側生成」方針を意味論パリティ規則まで拡張する。

- **text-local gate** と **pseudo band order** を、既存の `proto/spec`（`style_tags.inherit` / `element_kinds.carriesTextLocal` / `pseudo_states`）から `Tsubame/proto/generator` 経由で生成し、**DOM の適用前・Canvas の encode 前の両方**が同一の生成物を参照する。DOM Renderer の selector 正規表現による優先順抽出は廃止。
- **prop-op の dispatch**（どの property がどの op-kind に coerce されるか・各 op-kind がどの意味か）は `proto/spec` 正本から生成し、text-local gate / pseudo band order と同様に **Contract 内の生成物**とする。両 adapter は生成された dispatch を共有し、「DOM 書き込み」「Canvas enqueue」という**効果ハンドラ**だけを埋める（効果のみ Contract 外）。op-kind の二重 match は持たない。

規則・dispatch は spec 正本（生成物）、効果（DOM / Canvas）のみ adapter——という分担にする。

## Consequences

- パリティが **interface で検証**される。mock や test double が両経路で同一規則を共有でき、片側だけの取りこぼしが構造的に起きない。
- 新しい pseudo-state / text-local プロパティの追加が **spec 1 箇所の編集**で両 renderer に波及。
- **locality**：text-local gate・pseudo order・prop-op の意味がそれぞれ単一ソースに集中。
- Canvas は Hayate の棄却に「依存」しなくなり、無駄な encode が消える。

## Considered Options

- **protocol パッケージに手書き集約**：生成せず共有関数（手書き visitor）として置く。spec 正本と乖離しうる利点を捨てる。とくに prop-op dispatch を手書き visitor にする案は、他のパリティ規則だけ spec 正本・dispatch だけ手書き、という非対称を生むため却下。dispatch も spec 生成に揃える。
- **最小（Canvas に gate 追加のみ）**：text-local gate だけ両経路に入れる。pseudo band の二重実装と prop-op の分散が残る。却下。

## 関係

- ADR-0070 / ADR-0055（spec 正本からの両側生成）を延長。
- CONTEXT の「意味論パリティ」原則を実装規約に落とす。
