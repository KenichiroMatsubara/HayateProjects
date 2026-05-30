# CJK フォントフォールバック — .notdef 検出による動的ダウンロード

## Context

Canvas Mode では WASM 環境にシステムフォントが存在しないため、バンドルフォント以外のグリフは `.notdef`（□）で描画される。CJK 文字（ひらがな・カタカナ・漢字）を含むテキストを動的に表示するには、不足グリフを検出して適切なフォントをダウンロードする仕組みが必要になる。

Flutter Web（CanvasKit renderer）は同様の問題を「コードポイント未検出 → `fonts.gstatic.com` から Unicode range 単位の Noto サブセットをダウンロード → 再描画」で解決しているが、その実装はいくつかの設計を前提にしている。Hayate に Flutter の設計をそのまま持ち込めるかどうか検討した。

## 決定

### 1. 検出ポイント：`lower_glyph_runs` の .notdef スキャン

Parley シェーピング後の `lower_glyph_runs`（`text.rs`）で `glyph.id == 0` を検出し、対応するコードポイントを `Run::text_range()` で逆引きする。

**却下した代替案：fontique の query 介入（事前検出）**
fontique の query 段階に割り込めば、1 レイアウトパス分のシェーピングを節約できる。しかし：
- vendored crate への手入れが必要
- CJK は合字・異体字が少なく、事前スキャンとシェーピング後で精度差がほぼない
- 節約できるシェーピングは 1 回のみ。ネットワーク DL（数百 ms〜）が支配的コストであり、この 1 回は無視できる
- `.notdef` 検出は「実際に欠けたグリフ」を正確に把握できるため、不要なダウンロードを起こさない

対象スクリプトは **CJK（ひらがな・カタカナ・漢字）のみ**。アラビア語・インド系文字は合字処理が複雑で事前スキャンの精度が落ちるが、現時点ではスコープ外とする。

### 2. 責務の分割：Core がコードポイント → ファミリ名、各アダプタがファミリ名 → 調達

```
Core（Rust）: codepoint_font_family(cp: u32) -> Option<&'static str>
  └─ Unicode ブロックテーブル（安定したドメイン知識）
     Hiragana → "Noto Sans JP"
     Katakana → "Noto Sans JP"
     CJK Ideographs → "Noto Sans JP"
     …

Web Adapter（Rust）: family name → CDN URL → TTF fetch（ADR-0043）
Native Adapter（将来）: family name → OS フォントディレクトリ問い合わせ

アプリ: フォントを意識しない
```

Core は `.notdef` グリフを検出した時点でコードポイント → ファミリ名の変換を行い、`FetchFont { family: String }` イベントをキューに積む。URL も調達方法も知らない。各アダプタが `poll_events()` 内で `FetchFont` を受け取り、プラットフォームに応じた方法でフォントを取得する（詳細は ADR-0043）。

**`FontMissing { codepoints: Vec<u32> }` を却下した理由**
コードポイント → ファミリ名のマッピングはドメイン知識であり、すべてのプラットフォームアダプタで同じロジックを重複実装することになる。Core が Unicode ブロックテーブルを持つことで、アダプタはプラットフォーム固有の調達ロジックだけを持てばよい。

### 3. 通知：`FetchFont { family }` イベントを Core キュー → アダプタが処理

1 レイアウトパスで複数のテキスト要素が同じファミリを必要としても、`pending_font_fetches: HashSet<String>` によりイベントは 1 回だけ発火する。フォントが `register_font()` で登録されると `pending_font_fetches` からエントリが削除され、次にそのファミリの .notdef が検出された時点で再度 `FetchFont` が発火できる（通常は起きない）。

### 4. キャッシュ無効化：`fonts_dirty` フラグ → 全テキスト要素の `mark_dirty()`

`register_font()` は `fonts_dirty = true` を立てる。次の `compute_layout()` の冒頭で `fonts_dirty` を確認し、`kind.is_text_like()` な全要素に対して：
- `text_layout = None` / `content_layout = None` をクリア
- `taffy.mark_dirty(el.taffy_node)` を呼ぶ

これにより次の render パスで Taffy がテキスト要素を再計測し、新フォントでシェーピングされる。`fonts_dirty = false` にリセット。

**Flutter との比較：**
Flutter Web は `handleSystemMessage('fontsChange')` → `RenderParagraph.markNeedsPaint()` で、テキスト要素のみを再描画対象にする（full rebuild しない）。Hayate でも Taffy の dirty 追跡により **テキスト要素のシェーピングのみ** が再実行される点は同等。

### 5. 毎フレーム全量再構築は問題ない

Flutter が retained-mode 部分更新を採用しているのは **Skia の CPU ラスタライズが高コスト** だからであり、毎フレームの全描画を避けるための最適化である。

Hayate の `vello_bridge::build_scene()` は：
- Vello の GPU compute パイプライン（flatten → binning → coarse → fine）に Scene をエンコードするだけ
- シェーピング・レイアウト計算は一切なく、キャッシュ済みグリフ座標の構造体コピー
- UI スケール（要素数 数百）で数十 μs 以下

Vello の GPU は毎フレーム全シーンを並列処理する前提で設計されており、Unity・Unreal などのゲームエンジンと同じ方式である。GPU ネイティブレンダラーにおいて「毎フレーム全量」は正しい設計であり、Flutter 式の部分更新は不要かつ複雑化のコストが正当化されない（ADR-0006 参照）。

## 却下した代替案（全体）

- **バンドルフォントで全言語を網羅する**：フォントが数十 MB になり WASM バイナリが現実的でなくなる
- **HTML Mode のみで対処する**：Canvas Mode が主ターゲットであり、HTML Mode のブラウザ native フォントフォールバックに頼るのは Canvas Mode を捨てることになる
- **アプリが `load_font_from_url` を起動時に呼ぶ**：アプリが使う文字セットを事前に知っている必要があり、動的な文字入力（IME）に対応できない
- **JS 側が全マッピングを持つ（コードポイント → URL）**：あらゆるプラットフォームで同一のロジックを重複実装することになる。Unicode ブロックテーブルはドメイン知識であり Core に置くべき

## 影響

- `text.rs`: `codepoint_font_family()` 追加、`lower_glyph_runs()` に .notdef 検出ロジック追加、`TextLayout` に `missing_families: Vec<&'static str>` フィールド追加
- `tree.rs`: `Event::FetchFont { family: String }` 追加、`ElementTree` に `fonts_dirty: bool` と `pending_font_fetches: HashSet<String>` 追加、`register_font()` と `compute_layout()` を更新
- `element_renderer.rs`: `event_kind_fetch_font() -> f64 { 15.0 }` 追加、`encode_events()` に `FetchFont` ブランチ追加
- `demo-05.html`: `EV_FETCH_FONT` 定数追加、`FONT_URL_MAP` 追加、`processEvents()` に `FetchFont` ハンドラ追加
