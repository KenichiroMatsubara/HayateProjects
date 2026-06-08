# §2 Element Layer

Hayate の上位インターフェース。element tree の構築・Hayate CSS の設定・正本ツリーの所有。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### ELEM-01 — React Native 要素語彙
**規範文:** Element Layer は React Native 語彙（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）の6種を採用し、HTML タグ名（`div` / `span` / `p` 等）は使わない。
**出典:** ADR-0009
**状況:** ✅ — `kind.rs` の `ElementKind` で6種定義、タグ名なし。`CONTEXT.md`「Element」と一致。
**備考:** ADR-0009 自体は「DOM Adapter 廃止で不要化」と記すが、RN 語彙の選択は有効。

### ELEM-02 — 制限付きスタイル継承
**規範文:** スタイル継承はテキスト系3プロパティ（`color` / `font-size` / `font-family`）に限定し、`scene_build` で top-down に `InheritedStyle` を渡して解決する（明示値があれば上書き、なければ親値を継承）。
**出典:** ADR-0047
**状況:** ✅ — `scene_build.rs` の `InheritedStyle`（color/font_size/font_family）と `walk()` の継承解決。`Visual` は各 prop を `Option`。
**備考:** セレクタ・カスケード・スタイルシートは対象外。

### ELEM-03 — 単一正本 Document Tree（描画正本）
**規範文:** **描画・layout・hit-test の正本**は唯一とする。Canvas 経路では Hayate `ElementTree`、DOM Renderer 経路ではブラウザ DOM。`CanvasRenderer` / `DomRenderer` は構造を持たない（write-only）。例外として `tsubame-solid` は `solid-js/universal` の同期ツリー走査要件のため `TsubameNode` を**構造専用 reconcile index**（parent / 順序付き children / kind）として保持してよい — これは描画正本の複製ではない。
**出典:** ADR-0062（ADR-0057 を supersede）
**状況:** ✅ — Hayate 側は単一の描画正本（`tree.rs` の `ElementTree`、`elements: HashMap<ElementId, Element>`、各 Element が parent/children 保持。Canvas adapter は tree 参照のみ）。`CanvasRenderer`/`DomRenderer` の parentOf/childrenOf は撤去済み。`tsubame-solid` の `TsubameNode`（`node.ts:13–14` の parent/children、`renderer.ts` の走査）は ADR-0062 により**構造専用 reconcile index として正式採用**（solid-js/universal が VDOM を持たず batch 境界越しの正本を同期で読めないため不可避。diff しないので VDOM ではない）。
**備考:** [履歴] ADR-0057「JS 側に構造を一切持たない」は ADR-0062 が supersede（コスト分析: signal 経路 CPU +0、メモリ増分 ~70 B/node、却下案=ハンドル化+構造 eager は reconcile walk が O(walk) 同期 WASM）。shadow の `text` フィールド残課題は **ADR-0063（IFC・collapse 撤去、§5 TEXT-08）で閉じる** — text 内容が実 `text` span element に宿るため shadow は構造のみになる。tsubame-react / tsubame-vue は VDOM reconciler のため shadow 不要。

### ELEM-04 — Element Layer の変更モデル（eager / deferred）
**規範文:** `element_*` 系の変更操作は、Canvas Mode では `ElementTree` に即時反映（eager）、HTML Mode では Command キューに積み `render()` で一括フラッシュ（deferred）する。
**出典:** ADR-0037（Canvas eager）, ADR-0030（HTML deferred）
**状況:** ✅ — 実装と検証は §8 WEBA-02 / WEBA-03 に集約。
**備考:** [履歴] ADR-0030 は当初「両モード deferred」だったが ADR-0037 が Canvas のキューを撤去し、deferred は HTML Mode のみに scope 縮小。詳細は §8。

### ELEM-05 — text-as-element
**規範文:** テキストは常に tree 上の `text` element として表現し、その文字列は当該 `text` element 自身のプロパティ（`el.text`）として持つ（DOM の `<span>` + Text ノードを 1 要素に畳んだモデル）。`el.text` が宿るのは text-like 要素のみ — `Text`（内容）/ `TextInput`（placeholder）。`view` / `button` / `image` / `scroll-view` はテキストを子 `text` element で持ち、親へ集約しない。仮想 TextNode は使わない。
**出典:** ADR-0058
**状況:** ✅ — Canvas/DOM 両側でモデルは一貫（text element が文字列をプロパティとして持つ。button ラベルは子 text element）。`element_set_text` は core で `Text` / `TextInput` のみに制限し、非 text 要素への set は no-op（ADR-0058 の不変条件を canonical tree で構造化）。Tsubame solid は `createTextNode→createElement('text')+setText`、`isTextInTextCollapse` で text-in-text を畳む（仮想 TextNode 廃止済み）。回帰テスト `element_set_text_is_ignored_on_non_text_elements`。
**備考:** [解決 C-2.1] §2 旧評価の「Canvas=property / DOM=要素 で非対称」は誤読 — 両側とも text element が文字列をプロパティで持つ同型モデル。**[更新] leaf-string + collapse モデルは ADR-0063（§5 TEXT-08）が supersede** — `text` は inline formatting context となり、子 `text` は collapse でなく inline span として合成される。`text-as-element`（正の ID・仮想 TextNode なし）の核は維持。collapse 撤去で `tsubame-solid` shadow の `node.text` が消え ELEM-03/ADR-0062 の残課題が閉じる。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 4 | ELEM-01, 02, 03, 05 |
| 🟡部分 | 1 | ELEM-04 |
