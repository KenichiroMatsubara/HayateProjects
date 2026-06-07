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

### ELEM-03 — 単一正本 Document Tree
**規範文:** UI 構造の正本は唯一とする。Canvas 経路では Hayate `ElementTree` が正本。Platform Adapter は `ElementId` ハンドルのみ保持し、parent map / shadow tree を複製しない。
**出典:** ADR-0057
**状況:** ✅ — `tree.rs` の `ElementTree`（`elements: HashMap<ElementId, Element>`、各 Element が parent/children 保持）が単一正本。adapter は tree 参照のみ。
**備考:** Tsubame DOM Renderer 経路ではブラウザ DOM が正本（別経路、§11 TSUB-03）。

### ELEM-04 — Element Layer の変更モデル（eager / deferred）
**規範文:** `element_*` 系の変更操作は、Canvas Mode では `ElementTree` に即時反映（eager）、HTML Mode では Command キューに積み `render()` で一括フラッシュ（deferred）する。
**出典:** ADR-0037（Canvas eager）, ADR-0030（HTML deferred）
**状況:** ✅ — 実装と検証は §8 WEBA-02 / WEBA-03 に集約。
**備考:** [履歴] ADR-0030 は当初「両モード deferred」だったが ADR-0037 が Canvas のキューを撤去し、deferred は HTML Mode のみに scope 縮小。詳細は §8。

### ELEM-05 — text-as-element ★
**規範文:** テキストはツリー上の独立した `text` element として表現し、親への文字列集約や仮想 TextNode を使わない（`button` 直下の文字列も child `text` element）。
**出典:** ADR-0058
**状況:** 🟡 — `ElementKind::Text` は存在し text は要素になり得る。しかし `Element` は `text: Option<String>` フィールドも持ち、`scene_build` は text を要素自身の property として直接 emit する経路があり、ADR-0058 の「常に tree 上の text element」を Canvas 側で完全には満たしていない。Tsubame DOM Renderer 側は独立 text 要素を生成（非対称）。
**備考:** [要判断 C-2.1] Canvas（property 扱い）と DOM（独立要素）の非対称。ADR-0058 の徹底には Canvas 側の修正が要る。エスカレーション対象。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 3 | ELEM-01〜03 |
| 🟡部分 | 2 | ELEM-04, ELEM-05 |
