# CJK フォントフォールバック — .notdef 検出による動的ダウンロード

## Context

Canvas Mode では WASM 環境にシステムフォントが存在しないため、バンドルフォント以外のグリフは `.notdef`（□）で描画される。CJK 文字（ひらがな・カタカナ・漢字）を含むテキストを動的に表示するには、不足グリフを検出して適切なフォントをダウンロードする仕組みが必要になる。

Flutter Web（CanvasKit renderer）は同様の問題を「コードポイント未検出 → `fonts.gstatic.com` から Unicode range 単位の Noto サブセットをダウンロード → 再描画」で解決しているが、その実装はいくつかの設計を前提にしている。Hayate に Flutter の設計をそのまま持ち込めるかどうか検討した。

## 決定

### 1. 検出ポイント：`lower_glyph_runs` の .notdef スキャン

Parley シェーピング後の `lower_glyph_runs`（`text.rs`）で `glyph.id == 0` を検出し、対応するコードポイントを `Layout::GlyphRun::run().text_range()` で逆引きする。

**却下した代替案：fontique の query 介入（事前検出）**
fontique の query 段階に割り込めば、1 レイアウトパス分のシェーピングを節約できる。しかし：
- vendored crate への手入れが必要
- CJK は合字・異体字が少なく、事前スキャンとシェーピング後で精度差がほぼない
- 節約できるシェーピングは 1 回のみ。ネットワーク DL（数百 ms〜）が支配的コストであり、この 1 回は無視できる
- `.notdef` 検出は「実際に欠けたグリフ」を正確に把握できるため、不要なダウンロードを起こさない

対象スクリプトは **CJK（ひらがな・カタカナ・漢字）のみ**。アラビア語・インド系文字は合字処理が複雑で事前スキャンの精度が落ちるが、現時点ではスコープ外とする。

### 2. 通知：`FontMissing` イベントを `poll_events()` 経由で JS に渡す

```
FontMissing { codepoints: Vec<u32> }
```

1 レイアウトパスで検出した欠けコードポイントをまとめて 1 イベントに束ねる。個別発火では同一パスで 100 文字欠けていた場合に 100 イベントが出る。

Core は URL を知らない。`FontMissing` イベントを受け取った **JS 側が** コードポイントを見てどのフォントをダウンロードするかを判断し、`load_font_from_url()` を呼ぶ。これは ADR-0014（Platform Adapter scope）および ADR-0018（poll-events 通知）と整合する。

**却下した代替案：Core が CDN URL マッピングを持つ（Flutter 方式）**
Flutter は `fonts.gstatic.com` への依存をフレームワーク内部に持つ。Core にネットワーク依存とハードコード URL を持ち込むと、オフライン・イントラネット・CDN 変更に脆くなる。Hayate では URL マッピングをアプリ層の責務とする。

**却下した代替案：アプリが事前にフォントマニフェストを登録する**
`register_font_manifest([{ range: [0x3000, 0x9FFF], url: "..." }])` のような API でも実現できるが、`FontMissing` + JS-side マッピングの方が柔軟性が高くシンプル。

### 3. キャッシュ無効化：`fonts_dirty` フラグ → 全テキスト要素の `mark_dirty()`

`register_font()` は `fonts_dirty = true` を立てる。次の `compute_layout()` の冒頭で `fonts_dirty` を確認し、`kind == Text || kind == TextInput` の全要素に対して：
- `text_layout = None` / `content_layout = None` をクリア
- `taffy.mark_dirty(el.taffy_node)` を呼ぶ

これにより次の render パスで Taffy がテキスト要素を再計測し、新フォントでシェーピングされる。`fonts_dirty = false` にリセット。

**Flutter との比較：**
Flutter Web は `handleSystemMessage('fontsChange')` → `RenderParagraph.markNeedsPaint()` で、テキスト要素のみを再描画対象にする（full rebuild しない）。Hayate でも Taffy の dirty 追跡により **テキスト要素のシェーピングのみ** が再実行される点は同等。

ただし Hayate の `scene_build::build()` と `vello_bridge::build_scene()` は毎フレーム全量再構築であり、Flutter の retained-mode ペイントツリーに相当する部分更新はない。これは意図した設計である（次節参照）。

### 4. 毎フレーム全量再構築は問題ない

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

## 影響

- `text.rs:lower_glyph_runs` に `.notdef` 検出とコードポイント収集ロジックを追加する
- `Event` enum に `FontMissing { codepoints: Vec<u32> }` バリアントを追加する
- `ElementTree` に `fonts_dirty: bool` フィールドを追加する
- `register_font()` が `fonts_dirty = true` をセットする
- `compute_layout()` が `fonts_dirty` チェックで全テキスト要素を `mark_dirty()` する
- `poll_events()` のエンコードに `FontMissing` を追加する
- demo-05.html に `FontMissing` イベントハンドラと CJK フォント URL マッピングを追加する
