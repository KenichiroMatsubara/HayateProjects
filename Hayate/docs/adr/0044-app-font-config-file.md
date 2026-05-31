# アプリフォント設定ファイル — hayate.config.json

## Context

アプリが Inter・Roboto・カスタムブランドフォント等のプライマリフォントを使う場合、従来は `load_font_from_url()` を起動コードに手書きする必要があった。これはアプリごとの重複実装であり、宣言と実装コードが分離されない問題がある。

ADR-0043 で実装した `FetchFont` 自動 fetch は「.notdef 検出時のスクリプト補填フォント（Noto 系）」に限定されており、アプリのプライマリフォント（Inter 等）はカバーしない。.notdef が出ないからである（Parley がデフォルトフォントにフォールバックするため）。

## 決定

**アプリフォントを `hayate.config.json` に宣言し、`configure_fonts()` で preload する。**

### スキーマ

```json
{
  "fonts": [
    { "family": "Inter", "url": "https://cdn.example.com/Inter[wght].ttf" },
    { "family": "MyBrand", "url": "/fonts/brand.ttf" }
  ]
}
```

フィールドは `family`（登録名）と `url`（TTF/OTF URL）のみ。weight/style は fontique がフォントファイルのメタデータから自動検出するため明示不要。同 family 名で複数ファイルを登録すればウェイト・スタイル混在も動作する。

### API

```js
const cfg = await fetch('./hayate.config.json').then(r => r.json());
await renderer.configure_fonts(cfg.fonts);   // ← フレーム描画前にブロック
```

`configure_fonts(fonts: JsValue) -> Promise<()>` として両 Renderer に実装。

- **Canvas Mode**: `fetch_bytes()` → `tree.register_font()` でフォントコンテキストに登録
- **HTML Mode**: `fetch_bytes()` → `inject_font_face()` でブラウザに CSS `@font-face` として登録

### タイミング：blocking preload

`await` することで全フォントのロードが完了してから最初のフレームを描画する。
これにより FOUT（Flash of Unstyled Text）が起きない。

FetchFont によるスクリプト補填フォント（Noto 系）の遅延 fetch とは対照的。

| 種別 | フォント | タイミング | トリガー |
|---|---|---|---|
| アプリフォント | Inter, Roboto 等 | blocking preload | hayate.config.json |
| スクリプト補填 | Noto Sans JP 等 | background lazy | .notdef 検出 |

### 実装の選択

**直列 fetch を選んだ理由**: 起動時のフォント数は 2〜5 程度であり、並列化の効果より `futures::join_all` 依存追加のコストが上回る。必要になれば将来追加できる。

## 却下した代替案

- **`load_font_from_url()` を毎回書く**: アプリごとに同じコードを重複する。フォント URL の変更箇所が分散する。
- **WASM が config ファイルを自動探索する**: WASM はサーバーファイルを自分で読めない。JS が fetch して渡す必要がある。
- **init() の引数に config を渡す**: 初期化と font 宣言が密結合になり、config なしの init ができなくなる。config 省略可能にするより明示的な 2 ステップが明快。
- **フォント設定を package.json に混ぜる**: Hayate 固有の設定を npm の設定ファイルに入れると他ツールとの干渉リスクがある。

## 影響

- `element_renderer.rs`: `HayateElementRenderer` と `HayateElementHtmlRenderer` に `configure_fonts(fonts: JsValue)` async メソッドを追加
- `examples/web-demo/hayate.config.json`: アプリが宣言するフォント設定ファイルの雛形
- `examples/web-demo/demo-05.html`: renderer init 後に config を読み込んで `await configure_fonts()` を呼ぶ bootstrap パターンを追加
