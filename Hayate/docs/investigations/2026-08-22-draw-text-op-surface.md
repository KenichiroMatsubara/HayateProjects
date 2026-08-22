# `draw` にテキスト描画を足すための面の調査（Flutter `drawParagraph` 相当）

**Date: 2026-08-22** / **App: Tsubame Shogi（`Tsubame/examples/shogi-demo`）**

将棋GUI デモを作る過程で、`draw`（draw v1・ADR-0141）に**テキスト描画命令が無い**ことが
実アプリの制約として表面化した。ADR-0141 §21 はテキストを「封印ではなく後回し」と明記し、
encoding・enum 表・painter interface は「これらが契約破壊なしに生えること」を合格条件と
している。本書はその合格条件を実際に行使するために、**触る面と、残っている設計判断**を
洗い出したもの。実装は行っていない。

---

## 訂正（2026-08-22・実装後に追記）

**本書の §3 は、存在しない設計課題を 2 つ立てていた。** 実装（#732）で分かった実態を先に
書いておく。以下の本文は当時の記録としてそのまま残すが、次の 3 点は**誤り**である。

| 本書の記述 | 実態 |
| --- | --- |
| §3(a)「文字列の載せ方が唯一の新規設計」 | **前例が既にあった。** `SET_FONT_FAMILY` は長さ前置の UTF-8 バイトを 1 スロット 1 バイトで `styles: Float32Array` に直接埋めている（`Tsubame/proto/generated/codec.ts` の `encode_fontFamily`、decoder は `Hayate/proto/generator/src/lib.rs` の `variable_length` アーム）。draw も同じ形にすれば済み、`texts` のような別チャネルも dispatch の署名変更も要らなかった。採ったのはこれ |
| §3(b)「シェープが `text` 要素のレイアウトパスに深く結び付いている」 | **切り離す作業は要らなかった。** `pub fn build_text_layout(font_cx, layout_cx, text, font_size, max_advance, font_family, font_weight, font_style) -> TextLayout`（`crates/core/src/element/text.rs`）が既に公開されており、要素の内容でない単発ラベル（IME ツールバー）が `shape_label` 経由で既に使っていた。draw も同じ入口をそのまま呼ぶ |
| §3(c)「日本語字形は実行時に CDN から取るので将棋盤は必ず踏む」 | **踏まない。** `NotoSansJP.ttf` は `include_bytes!` で core にバンドルされ、既定 family かつ sans-serif の総称として登録されている（`crates/core/src/element/text_shaper.rs`）。漢字は初回フレームからネットワーク無しで出る。CDN が要るのは韓国語・簡繁体・記号・絵文字 |

**要するに、発明する部分は無く、繋ぐだけだった。** 段取りは要素テキストと同一で、
「レイアウトパスでシェープ → scene build で `intern_text_run` → walk は塗るだけ」。
walk（`render_scene_graph`）が `&impl SceneRead` の immutable 経路で interner を持たない
以上、この順序しか取れない — そしてそれは要素テキストが既に取っている順序である。

実装で本当に難しかったのは §3 が挙げていない 3 点で、いずれも**例外を出さず絵だけが壊れる**:

1. `sweep_resources` の生存判定が `NodeKind::TextRun` しか見ておらず、DrawList 中の
   `TextRunId` が参照中に回収される（→ `StaleTextRun`）
2. `fonts_dirty` の再シェープ対象が `is_text_like()` で絞られており、draw を運ぶ `view` が
   漏れる（届いたフォントが反映されず豆腐で固定される）
3. レイヤーのラスタ境界が path verb から面積を出すので、verb を持たない文字は実測の
   レイアウト寸法を持ち回る必要がある

§4（プロトコル版数）と「触る面」の表は概ね正しかった。

---

## なぜ実アプリで効いたか

将棋盤は「即時2D描画 ＋ 文字 ＋ 回転」を同時に要求する:

- 駒の五角形は `DrawCanvas` で描ける。`rotate` があるので**後手向きに 180° 回せる**。
- 駒の漢字は `DrawCanvas` では描けない。`HayateCssStyle` にも `transform` / `rotate` は
  無いので、要素側の `text` で重ねても**回せない**。

結果として当時の shogi-demo は「五角形は下向き・漢字は正立で別色」という、紙の将棋から
逸脱した表現を採っていた。draw にテキストが生えれば `translate → rotate(π) → 駒文字` で
正しい向きに描ける。（→ #732 で解消。駒は `paintPiece` 1 箇所の描画単位になり、先後は
反転フラグ 1 つで表す。）

## 判明したこと

### 1. 塗る側は既に在る（追加コスト小）

`ScenePainter` は既に `draw_text_run(x, y, color, text_run: TextRunId, resources: &SceneResources)`
を持つ（`crates/core/src/render/painter.rs:239`）。vello / tiny-skia / skia の各実装も揃って
いるので、**バックエンドごとのグリフ描画を新規に書く必要は無い**。

`SceneGraph` 側にも `intern_text_run(data: TextRunData) -> TextRunId`
（`crates/core/src/node.rs:476`）があり、シェープ済みの run を登録して id を得る口は開いている。

### 2. op 表の追加は素直（生成系は表駆動）

op 語彙の正本は `Hayate/proto/spec/draw_ops.json`（20 個・値 0〜19）。各エントリは
`name` / `value` / `drawRole`（`path-verb` | `draw-command`）/ `params` / `variable_length` を持つ。
`FILL`(3) と `STROKE`(11) が**可変長 op の前例**（`paint_len` 個の tagged field がインラインで続く）。

TS 側の生成器は表駆動で、未登録の新 op は camelCase へフォールバックする設計
（`Tsubame/proto/generator/gen-recorder.mjs`: 「未登録の新 op は camelCase にフォールバック
— 手書き不要」）。Rust 側の定数も生成物（`Hayate/proto/generated/protocol.rs:1466-1485`）。

→ **テキスト op は `value: 20` / `drawRole: "draw-command"` / `variable_length: true` の追記**
という形になる。

### 3. 本当に新しいのは「文字列をどう載せるか」と「いつシェープするか」

**(a) 文字列の encoding.** draw の wire チャネルは `draws: Float32Array`（ADR-0141 §18・
`texts` と同格）。既存 op はすべて数値なので、文字列の載せ方が唯一の新規設計になる。

| 案 | 中身 | 得失 |
| --- | --- | --- |
| A. コードポイントをインライン | 可変長 op のスロットに UTF-32 を f32 として並べる | display list が自己完結し `shouldRepaint` のキャッシュと相性が良い。将棋の駒は 1〜2 文字なので量も問題にならない |
| B. 文字列サイドテーブル | op には索引だけ入れ、実体は `texts` 同様の別チャネル | 長文で有利。`texts` チャネルという前例がある |

**(b) シェープの位置。** Flutter は `ParagraphBuilder.layout()` と `Canvas.drawParagraph()` を
分けている（シェープが高価でキャッシュしたいため）。Hayate も `TextRunId` + `SceneResources`
という同じ分け方を既に持つので、**op ストリームに生文字列を毎フレーム流して毎回シェープする
形にしてはいけない**。

しかし現在シェープを行う `lower_glyph_runs`（`crates/core/src/element/text.rs`）は
`text` 要素のレイアウトパスに深く結び付いている:

- parley によるシェープ、フォント解決、**カバレッジフォールバック**
  （`font_coverage::family_for_codepoint` で不足ファミリを収集）
- その不足ファミリが `ElementTree::drive_font_requests` を通じて**非同期のフォント取得**を駆動する
- synthesis（合成ボールド／斜体）と decoration の生成
- 生成した run は `retain_text_runs(&live)` の sweep 対象（生存管理が要る）

→ draw から使うには「文字列 + family + size + weight を渡すとシェープ済み run を返す」
入口を、レイアウトパスから切り離して用意する必要がある。

**(c) フォント未ロード時の扱い（要決定）。** `text` 要素は非同期取得を待てるが、draw の
replay は**そのフレームで絵を出す必要がある**。日本語字形は実行時に CDN から取る
（`crates/platform/web/fonts.json` → `hayate-fonts.pinara.workers.dev`）ので、将棋盤は
この経路を必ず踏む。第一候補は `text` 要素の既存の待ち方に揃えること。

### 4. プロトコル版数は上がる

`HOST_PROTOCOL_VERSION = manifest.version`（`Hayate/host/src/index.ts:61`）で、
`manifest` は `Hayate/proto/spec/manifest.json`（現在 `"version": 2`、`draw_ops` を section に含む）。
Torimi は起動時にホストの decoder 版数とバンドルの encoder 版数を突き合わせ、
不一致なら **mount せずエラー UI を出す**（#530）。

→ op を足せば版数は 3 に上がり、**出荷済みの Android ホスト（v2）は v3 バンドルを拒否する**。
ただし Demo Endpoint の配信はもともと `torimi-android-v*` タグと lockstep（Torimi ADR-0003）
なので運用上の追加負担は無い。ADR-0141 が保証するのは**エンコーディングの拡張可能性**で
あって旧ホストの前方互換ではなく、テキストを要求する画面は旧ホストでどのみち正しく描けない
以上、版数を上げて明示的に弾くのが正直な扱いになる。

## 触る面（まとめ）

| 層 | 対象 | 備考 |
| --- | --- | --- |
| 正本 | `Hayate/proto/spec/draw_ops.json`、`manifest.json`（version 2 → 3） | 生成物は手で触らない |
| 生成 | `Tsubame/proto/generator/gen-{recorder,draw-canvas,codec}.mjs` の出力、`Hayate/proto/generated/protocol.rs` | `pnpm check:proto` / `pnpm check:wire-contract` が門番 |
| Core | draw display list の walk → シェープ → `intern_text_run` → `draw_text_run` | **ここが本体**（上記 3(b)(c)） |
| バックエンド | vello / tiny-skia / skia | `draw_text_run` 実装済みのため追加コスト小 |
| DOM Renderer | canvas2d replay に `fillText` | Tsubame ADR-0014 §20 が「2D はネイティブ」として想定済み |

## 実証の置き場所

`Tsubame/examples/draw-gallery` が draw 機能の正規の実証場（README: 「draw 機能 v1
（PRD #723 / ADR-0141）の集大成デモ」、painter 5 種を Hayate / DOM の両経路で表示）。
`text-sampler` と `rotated-text` の painter を足し、既存 e2e（`e2e/hayate.spec.ts` /
`e2e/dom.spec.ts`）と同型に両経路で確かめるのが素直。既存 5 種の絵が変わらないことも同時に見る。

## 併せて見つかった Flutter とのギャップ（本書のスコープ外・別途起票）

| Flutter | Hayate | 将棋GUIでの影響 |
| --- | --- | --- |
| `Transform` widget | style に `transform` / `rotate` が無い | draw にテキストが生えれば駒の回転は不要になる |
| `TextAlign` | style に `textAlign` が無い | flex の `justifyContent` で代替した |
| `Canvas.drawImage` | `DrawCanvas` に無い（`image` *要素*はある） | 影響なし |
| `Canvas.saveLayer` | 無い | 影響なし |
