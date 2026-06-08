# text element を inline formatting context にする（ADR-0058 を一部 supersede）

**Status: accepted（ADR-0058 の leaf-string/collapse 部分を supersede、ADR-0005 を拡張）**

**Date: 2026-06-07**

## Context

ADR-0058 は「text は常に Hayate `text` element（正の `ElementId`）」とし、`text` を**単一文字列の leaf**として `el.text` に持ち、Solid の text-in-text を**親へ `setText` 集約（collapse）**することで inline text モデル→leaf モデルを橋渡しした。

この leaf-string + collapse モデルには二つの欠陥がある。

1. **inline styled text を表現できない。** collapse は親文字列を上書きするだけで 区間ごとのスタイルを失う。単一 `text` element にも per-range スタイル API が無い（`text.rs` の `build_text_layout` は `font_family`/`font_weight` を element 全体に1つだけ取る）。そのため「キーワードは太字青・文字列リテラルは緑」のような**シンタックスハイライトされた1行**＝複数 styled inline text が1つの inline flow として整形・折り返しされるもの——を描けない。これはコードエディタ級アプリの必須能力（本基盤の目標アプリ像）。
2. **collapse が `tsubame-solid` shadow に `node.text` carry を強いる**（ADR-0062 の残課題）。

`<text>` は本質的に inline formatting context（IFC）であり、ブラウザの `<span>` / RN の nested `<Text>` と同じく、subtree を1つの inline 整形単位として shape すべきである。Parley の `ranged_builder`（`text.rs:120`）は styled range を元々サポートし、`lower_glyph_runs`（`text.rs:154`）は既に run 毎に `TextRunData` を吐く。整形機構は揃っている。欠けているのは element モデルとの統合。

## Decision

`text` element を **inline formatting context（IFC）** とする。**element を2クラスに分ける。**

### 境界規則

- **IFC root** = 親が `text` でない `text` element。**1つの Taffy leaf**（box）。measure は subtree 全体を整形した Parley layout から。
- **inline text element** = 親が `text` の `text` element。**Taffy box にならない**。親 IFC の Parley ranged layout 内の **styled range** になる。
- それ以外（`view` / `button` / `image` / `scroll-view`）は従来どおり block box。RN 語彙では文字列は必ず `text` に包まれる（ADR-0058: button ラベルも子 `text`）ため、IFC root は常に `text` element に限定でき、view/button を IFC root にする必要はない。
- IFC root の内容 = 自身の `el.text`（あれば）＋ 子の inline text element を document 順に連結。inline text element 数 0・自身 `el.text` 設定 = 現行 leaf の縮退ケース。

### InlineText seam（hayate-core 内部、host 契約ではない。ADR-0054 `ScenePainter` と同格）

- interface（小）: `shape(ifc_root, available_width) -> (Layout, RangeMap)`。再 shape は dirty 時のみ。
- 裏（大）: subtree walk・text 連結・区間ごとの style/brush の range push・line break・run lowering（`build_text_layout` を ranged 区間 対応に拡張）・hit-test の **byte-range → ElementId** マップ・AccessKit range。
- `text.rs` の現 leaf 整形は inline text element 数=1 の特殊化として吸収。

### MVP スコープ

- **inline atom（`text` 中の inline `image`/icon）は後続。** MVP は inline text element のみ。
- **`text-input` は leaf editable のまま**（自前の編集文字列、子の inline text element なし）。表示 `text` のみ IFC。
- **inline text element のスタイルは text 系プロパティのみ**（font-family / size / weight / style / color / decoration）。box 系（width / padding / flex / background）は inline text element では MVP 無視。

## Consequences

- **Taffy 構築**: `is_inline(id)`（`text` かつ祖先に IFC root）なら Taffy ノードを作らない。IFC root は measure 関数で subtree を整形する leaf。この inline text element 除外・IFC-leaf 化は **Taffy 投影の責務（ADR-0064 の `TaffyProjection` が lazy reconcile で実施）**。`el.taffy_node: Option`（inline text element は None）。
- **dirty 伝播**: inline text element への `setText` / `setStyle` / append / remove は inline text element ではなく **IFC root を再 shape 対象**にする（inline text element に layout が無い）。mutation 経路で `ifc_root(id)`（親が `text` である限り遡上、O(inline 深さ)）を求め、**`shape_dirty.insert(root)` ＋ Taffy `mark_dirty(root.taffy_node)`**。**inline 変化は Taffy 構造を変えないので `structure_dirty`（ADR-0064）には触れない**。`run()` で shape_dirty の IFC root を InlineText で再 compose（プリパス）→ measure closure が line-break。（color/decoration は geometry 不変だが、初期実装は常に再 measure で単純化し、metric 非影響プロパティのスキップは後続最適化。）
- **scene_build**: IFC root が合成 layout の glyph runs を emit。inline text element 自身は emit しない。区間ごとの color は Parley brush（`TextBrush=[u8;4]`）を range で push。
- **hit-test（二段・畳み込み）**: stage 1（box）は inline text element が `layout_cache` に矩形を持たないため `hit_test_walk` が**自然に IFC root で止まる**（追加コード不要）。stage 2 を `hit_test` に**畳み込む**：IFC root を返す直前に Parley 点→byte hit-test（`local = 点 − text_origin`）→ `RangeMap`（byte→最深 inline text element、#1 で構築）で解決し、**inline text element を直接返す**。point が IFC root 自身の text／空白なら fallback で root。これで event targeting・hover・focus が一貫して「最特定 element」を受け取る（現行「最深 box を返す」の自然な延長）。`text_origin` は IFC root box ＋ padding から算出。`text-input` は leaf なので box hit で自身を返す（stage 2 不要）が、stage 2 の「Parley 点→byte」プリミティブは text-input のクリック→カーソル配置（`cursor_byte_index`）と共有。inline text element が `:hover` を持つ場合、hover で IFC root が `shape_dirty`（#2 と整合）。
- **DOM 経路はタダ**: Tsubame DOM Renderer / Hayate HTML Mode は nested `text`→nested `span` でブラウザが IFC をネイティブ整形。**実装コストは Canvas Mode(Parley) のみ。**
- **AccessKit**: IFC の合成 text を Parley `LayoutAccessibility::build_nodes` で range 化（PLAT-04 の下流）。
- **`tsubame-solid` の collapse 削除**: `isTextInTextCollapse` / `node.text` / collapse 分岐（`renderer.ts:126–128,144–146,61–69`）を撤去。`createTextNode`→inline text element + `setText`、text-into-text 挿入は単なる `appendChild`。→ **shadow から `node.text` が消え、ADR-0062 の「構造のみ」残課題が閉じる。**

## 関係

- ADR-0058: 「text は常に Hayate `text` element（正の ID・仮想 TextNode なし）」の核は**維持**。「leaf-string + 親 collapse」モデルのみ本 ADR が **supersede**。
- ADR-0005（Linebender stack）: Parley `ranged_builder` の styled range / brush を IFC 整形に使う形で**拡張**。

## Considered Options

- **leaf-string + collapse 維持（ADR-0058 現状）**: inline styled text 不可。エディタ用途で頭打ち。却下。
- **子 text を別 Taffy box として materialize（2a）**: inline 整形（折り返し・kerning・bidi）が inline text element 境界で壊れる。text-grade UI に不可。却下。
- **IFC（本 ADR / 2b）採用。** 子の inline text element を inline range とし、subtree を1整形単位に。
