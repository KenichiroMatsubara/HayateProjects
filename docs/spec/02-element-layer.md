# §2 Element Layer

Hayate の上位インターフェース。element tree の構築・Hayate CSS の設定・正本ツリーの所有。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### ELEM-01 — React Native 要素語彙
**規範文:** Element Layer は React Native 語彙（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）の6種を採用し、HTML タグ名（`div` / `span` / `p` 等）は使わない。
**出典:** ADR-0009
**状況:** ✅ — `kind.rs` の `ElementKind` で6種定義、タグ名なし。`CONTEXT.md`「Element」と一致。
**備考:** ADR-0009 自体は「DOM Adapter 廃止で不要化」と記すが、RN 語彙の選択は有効。

### ELEM-02 — テキスト継承は text-local ＋ ambient Default Text Style の2チャネル
**規範文:** テキスト継承は2チャネル。**(1) 通常 text スタイル**（`color`/`font-size`/`font-family`/`font-weight`/`font-style`/`text-decoration`）は **text→text（IFC 内 inline text 連鎖）のみ**継承し block を貫通しない（`view` に置いても text に漏れない）。`InlineText`（ADR-0063）が解決。**(2) ambient Default Text Style**（`default-color`/`default-font-family`/`default-font-size`/`default-font-weight`）は任意 element に設定でき **block を貫通**して既定値を供給する（Flutter `DefaultTextStyle` 相当）。`scene_build.walk` の top-down 機構が運ぶ。解決順: 自身明示 → text 祖先継承 → ambient 既定 → ハード既定。プロパティ名は CSS 準拠、適用は Flutter 寄せ（LLM 予測可能性）。
**出典:** ADR-0065（ADR-0047 を supersede）
**状況:** ✅ — `ambient_defaults.rs`（ch2 ambient `default-*`・block 貫通）、`inline_text.rs`（ch1 text→text 継承＋ambient フォールバック）、`scene_build.rs`（`AmbientDefaults` threading）。`style_tags.json` に `font-style`/`text-decoration`/`default-*` 追加済み。回帰テスト `text_inheritance.rs`。
**備考:** [履歴] ADR-0047 は「全 element 横断継承＝Flutter」と記したが誤記（実物 Flutter は DefaultTextStyle のみ貫通）。ADR-0065 が実物 Flutter の2チャネルに是正。セレクタ・カスケード・スタイルシートは対象外。

### ELEM-03 — 単一正本 Document Tree（描画正本）
**規範文:** **描画・layout・hit-test の正本**は唯一とする。Canvas 経路では Hayate `ElementTree`、DOM Renderer 経路ではブラウザ DOM。`CanvasRenderer` / `DomRenderer` は構造を持たない（write-only）。例外として `tsubame-solid` は `solid-js/universal` の同期ツリー走査要件のため `TsubameNode` を**構造専用 reconcile index**（parent / 順序付き children / kind）として保持してよい — これは描画正本の複製ではない。
**出典:** ADR-0062（ADR-0057 を supersede）
**状況:** ✅ — Hayate 側は単一の描画正本（`tree.rs` の `ElementTree`、`elements: HashMap<ElementId, Element>`、各 Element が parent/children 保持。Canvas adapter は tree 参照のみ）。`CanvasRenderer`/`DomRenderer` の parentOf/childrenOf は撤去済み。`tsubame-solid` の `TsubameNode`（`node.ts:13–14` の parent/children、`renderer.ts` の走査）は ADR-0062 により**構造専用 reconcile index として正式採用**（solid-js/universal が VDOM を持たず batch 境界越しの正本を同期で読めないため不可避。diff しないので VDOM ではない）。
**備考:** [履歴] ADR-0057「JS 側に構造を一切持たない」は ADR-0062 が supersede（コスト分析: signal 経路 CPU +0、メモリ増分 ~70 B/node、却下案=ハンドル化+構造 eager は reconcile walk が O(walk) 同期 WASM）。shadow の `text` フィールドは ADR-0063（§5 TEXT-08）で撤去済み — `TsubameNode` は `{id, kind, parent, children, events}` のみ。tsubame-react / tsubame-vue は VDOM reconciler のため shadow 不要。

### ELEM-04 — Element Layer の変更モデル（eager / deferred）
**規範文:** `element_*` 系の変更操作は、Canvas Mode では `ElementTree` に即時反映（eager）、HTML Mode では Command キューに積み `render()` で一括フラッシュ（deferred）する。
**出典:** ADR-0037（Canvas eager）, ADR-0030（HTML deferred）
**状況:** ✅ — 実装と検証は §8 WEBA-02 / WEBA-03 に集約。
**備考:** [履歴] ADR-0030 は当初「両モード deferred」だったが ADR-0037 が Canvas のキューを撤去し、deferred は HTML Mode のみに scope 縮小。詳細は §8。

### ELEM-05 — text-as-element
**規範文:** テキストは常に tree 上の `text` element として表現し、その文字列は当該 `text` element 自身のプロパティ（`el.text`）として持つ（DOM の `<span>` + Text ノードを 1 要素に畳んだモデル）。`el.text` が宿るのは text-like 要素のみ — `Text`（内容）/ `TextInput`（placeholder）。`view` / `button` / `image` / `scroll-view` はテキストを子 `text` element で持ち、親へ集約しない。仮想 TextNode は使わない。
**出典:** ADR-0058
**状況:** ✅ — Canvas/DOM 両側でモデルは一貫（text element が文字列をプロパティとして持つ。button ラベルは子 text element）。`element_set_text` は core で `Text` / `TextInput` のみに制限し、非 text 要素への set は no-op（ADR-0058 の不変条件を canonical tree で構造化）。Tsubame solid は `createTextNode→createElement('text')+setText`、text-in-text は `appendChild` で各 `text` element に `setText`（ADR-0063・仮想 TextNode / collapse 廃止済み）。回帰テスト `element_set_text_is_ignored_on_non_text_elements`。
**備考:** [解決 C-2.1] §2 旧評価の「Canvas=property / DOM=要素 で非対称」は誤読 — 両側とも text element が文字列をプロパティで持つ同型モデル。**[更新] leaf-string + collapse モデルは ADR-0063（§5 TEXT-08）が supersede** — `text` は inline formatting context となり、子 `text` は inline text element として IFC 合成される。`text-as-element`（正の ID・仮想 TextNode なし）の核は維持。collapse 撤去で `tsubame-solid` shadow の `node.text` が消え ELEM-03/ADR-0062 の残課題が閉じる。

### ELEM-06 — 実効スタイル解決は単一 resolver、query で露出
**規範文:** per-element の実効スタイル解決（継承 ch1 text-local ＋ ch2 ambient → 自身明示 → pseudo `focus<hover<active`）は1つの shared resolver に集約し、`ElementTree::element_effective_visual(id) -> Visual` で query 露出する。`scene_build`（box/visual・継承 threading）・`InlineText`（inline text element の text-style）・query が同一 resolver を共有する（継承 context の取得経路のみ caller で異なる）。effective visual に限定（layout prop は Taffy 領分で除外）。
**出典:** ADR-0067（ADR-0056/0065/0063 を統合）
**状況:** ✅ — `effective_visual.rs` の `resolve_effective`（継承 ch1+ch2 → 自身 → pseudo）を `scene_build`・`inline_text`・`ElementTree::element_effective_visual` が共有。query 露出済み。回帰テスト `effective_visual.rs`。
**備考:** test surface が Visual になり `:hover` を1呼び出しで検証可能。hit-test 精緻化・AccessKit・debug が同 query を共有。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 6 | ELEM-01〜06 |
| 🟡部分 | 0 | — |
| ⬜未実装 | 0 | — |
