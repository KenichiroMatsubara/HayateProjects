# tsubame-solid は構造専用 shadow tree を保持する（ADR-0057 を覆す）

**Status: accepted（ADR-0057 を supersede）**

**Date: 2026-06-07**

## Context

ADR-0057 は「文書構造の正本は backend 一箇所」とし、`tsubame-solid` の `TsubameNode` から構造ミラー（`parent` / `children`）を撤去して `ElementId` ハンドル化し、ツリー走査を backend に委譲すると decide した。その前提は「shadow tree は React VDOM 相当の冗長な複製である」だった。

再検討の結果、この前提は二点で誤りであることが判明した。

### 1. shadow tree は VDOM ではない

VDOM は「diff を取るための仮想コピー」。`TsubameNode.children` は **diff されない**。Solid は fine-grained reactive で vtree を比較しない。`getNextSibling` 等は仮想コピーではなく、ホストの実体ツリーを歩くための API。diff 対象が存在しない以上、これは VDOM ではなく、**reconciler の作業用ホストツリー**である。

### 2. solid-js/universal は VDOM を持たないがゆえに「同期で読めるホストツリー」を要求する

`createRenderer<T>` の `T` はホストのノード型であり、`getParentNode` / `getFirstChild` / `getNextSibling` は `T` を歩く。`T` 自身が parent/children を持つ retained オブジェクトでなければならない。

| reconciler | reconcile 時にホスト構造を読むか | ホスト要件 |
|---|---|---|
| React Fiber / Vue VDOM | 読まない（自前 vtree を diff） | write-only・batch・境界越しで可 |
| **Solid universal** | **同期で読む**（vtree が無い） | **同期で読める retained ツリーが必須** |

in-process なホスト（DOM `Element`、three.js `Object3D`）はノードが JS 内にあり parent/children をネイティブに持つため、ツリーは1本・複製ゼロでこの問題が発生しない。

Tsubame の差異は、**正本ツリーが WASM の batch 境界の向こうにある**こと（`apply_mutations` 1回/frame、TSUB-04 / ADR-0052）。JS から同期到達できない。正本ツリーが境界の向こうにあるとき、近側に reconcile 用インデックスを置くのは古典的に不可避（cf. React Native の shadow nodes）。

帰結として、**shadow tree は「Solid が VDOM を拒否した代償」**であり、`tsubame-solid` 固有である。VDOM を持つ tsubame-react / tsubame-vue は構造を読み返さないため shadow tree を必要としない。

## Decision

**ADR-0057 を覆す。** `tsubame-solid` は `TsubameNode` を **構造専用 reconcile index** として保持する。

- shadow tree が持つもの: `id`・`elementKind`・`parent`・順序付き `children`（= 構造）、および listener ハンドル（`events`。メモリ削減のため遅延生成）。
- shadow tree が持たないもの: **描画の正本**（text 内容・style・layout・hit-test）。これらは引き続き backend（Canvas: Hayate `ElementTree` / DOM Renderer: ブラウザ DOM）が唯一の正本。

ADR-0057 の有効な核（「正本は単一」）は **「描画・layout・hit-test の正本は単一」** に re-scope する。reconcile の作業インデックスは描画正本の複製ではなく、正当な JS 資産である。

## コスト分析（再 litigate しないため記録）

純粋 signal をベースラインとした計測感（order-of-magnitude）:

- **CPU**: signal 値更新の経路で shadow は読みも書きもされない（`setProperty` / `replaceText` は `node.id` のみ参照） → **+0**。構造 op（mount / `<For>` / `<Show>`）でのみ配列 op O(1)/node を追加し、隣で必ず払う WASM mutation の <1%。
- **メモリ**: TsubameNode ~200 B/node。ハンドルのみモデルに対する**増分は ~70 B/node**（`children[]` + `parent`）。仮想化前提の現実 live 数（≤20k）で **<1 MB**。Hayate 側 ~1 KB/element（`Element` + Taffy node + scene node）の前では誤差。
- メモリの主因は `events` Map（両モデル共通）。遅延生成が効く。

### 却下した代替（ハンドル化 + 構造 eager + 同期 WASM walk）

「JS 構造ゼロ」を達成するが、reconcile walk が **O(walk) 回の同期 WASM round-trip** を要する（純 JS 配列走査の shadow に対し大 `<For>` で ~100〜1000倍）。これはエディタ級アプリのホットパス（大量行・補完候補・ファイルツリーの filter/scroll 再 reconcile）で最も効く。**locality を買い戻すために reconcile leverage を支払う**取引であり、本基盤の目標アプリ像に不利。

→ shadow tree 維持は CPU・メモリともほぼタダで、reconcile が最速。支払うのは **locality（構造 owner が2つ＝整合性税）** のみ。この整合性税を、reconcile を WASM 境界から外す対価として受容する。

## Consequences

- `tsubame-solid` の `TsubameNode`（`parent` / `children`）は**正式採用**（撤去しない）。`renderer.ts` の `insertNode` / `removeNode` は構造を backend mutation と整合させ続ける（整合性税を受容）。
- ADR-0057 の consequence のうち**正しいものは維持**:
  - `CanvasRenderer` / `DomRenderer` の `parentOf` / `childrenOf` 撤去は維持（renderer は write-only。構造を読むのは Solid の host である adapter のみ）。
  - 仮想負 ID TextNode の廃止は維持（ADR-0058）。
  - `removeChild` の subtree 片付けは backend が担う。
- **text の扱い（残課題）**: 描画正本の text は backend。shadow の `text` フィールドは text-in-text collapse（`createTextNode` の値 → `insertNode` collapse）のための **reconcile-transient な carry** に限定し、第二の正本とはしない。`text` を構造専用から外しきる小リファクタ（collapse 機構の見直し）はフォローアップ。
- tsubame-react / tsubame-vue は shadow tree を持たない（VDOM reconciler・write-only host）。
- 関連: ADR-0053（Element Document Runtime）、ADR-0052 / TSUB-04（apply_mutations batch）、ADR-0058（text-as-element）。

## 関係

ADR-0057 は **superseded**。その「描画正本は単一」という核は本 ADR が継承し、「JS 側に構造を一切持たない」部分のみを覆す。
