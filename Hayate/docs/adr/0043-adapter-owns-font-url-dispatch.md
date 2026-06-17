# アダプタが フォント URL ディスパッチを所有する

## Context

ADR-0042 で `FetchFont { family }` イベントを Core が発火し、JS（アプリ層）がファミリ名 → URL のマッピングを持つ設計を選んだ。しかしこの設計には問題がある。

- CDN URL という「プラットフォームの知識」がアプリコードに漏れる
- すべてのアプリが同一の URL マッピングをコピーする必要がある
- Flutter Web はエンジン（= adapter 相当）が `fonts.gstatic.com` への fetch を完全に隠蔽しており、アプリ開発者はフォント URL を一切書かない

## 決定

**ファミリ名 → URL のマッピングと非同期 fetch を web adapter（Rust/WASM）が所有する。**

```
Core（Rust）:        codepoint → family name （Unicode ブロックテーブル）
                      ← 安定したドメイン知識。プラットフォーム非依存。

Web Adapter（Rust）: family name → CDN URL → TTF を fetch
                      ← プラットフォーム固有の調達ロジック。

Native Adapter（将来）: family name → OS フォントディレクトリから読む
                         OS にない場合はローカルバンドル or 別ソース。

App（JS/Dart/etc.）: フォント URL を一切知らない。透過的。
```

### 実装

`HayateElementRenderer::poll_events()` が `FetchFont` イベントを interceptして：

1. `builtin_font_url(family)` でビルトイン URL を引く
2. URL があれば `wasm_bindgen_futures::spawn_local` で非同期 fetch を開始
3. fetch 完了後、`Rc<RefCell<Vec<(String, Vec<u8>)>>>` の `font_queue` に積む
4. 次の `poll_events()` 冒頭で `font_queue` をドレインして `tree.register_font()` に渡す
5. `FetchFont` イベントはアプリに渡さない（visible イベントから除外）

> **更新（ADR-0106 / issue #343）**：fetch *失敗*時は `console.warn` するだけだったため、ファミリが恒久的に欠落した。現在はバックオフ後に `font_failure_queue` へ積み、`render()` が `tree.font_fetch_failed()` へドレインして Core に失敗を伝える。Core が有限予算でリトライ／断念を判断する。

ビルトイン URL テーブル（`builtin_font_url`）は web adapter の Rust コードに持ち、TTF/OTF のみ登録する（fontique/skrifa は WOFF2 を解釈しないため）。

**`Rc<RefCell<...>>` を選んだ理由**：WASM は単一スレッドのため `Arc<Mutex<...>>` 不要。`spawn_local` のクロージャは `'static` を要求するが `Rc` は `clone()` で渡せる。wasm-bindgen の `#[wasm_bindgen]` struct も `Send` 不要なので問題ない。

## 却下した代替案

- **アプリ（JS）が FONT_URL_MAP を持つ**：プラットフォーム知識がアプリに漏れる。アプリごとに同一コードを重複する。ADR-0042 の初期案だったが差し戻し。

- **Core が URL を持つ（Flutter 完全踏襲）**：CDN URL という変更頻度の高い情報が Core に入る。オフライン・イントラネット環境に弱い。ADR-0042 で既に却下。

- **JS コールバックを渡す**：アダプタが `onFetchFont(callback)` を公開し JS に委譲する案。アプリが URL を書かなくて済むが、デフォルト挙動をアダプタ内で完結できないため採用しない。

## 影響

- `element_renderer.rs`：`FontQueue` 型、`builtin_font_url()`、`font_queue` フィールド追加。`poll_events()` で FetchFont を interceptして `spawn_local` + `font_queue` パターンで自律 fetch。
- `event_kind_fetch_font()` WASM export を削除（アプリに見せない）。
- `encode_events()` の `FetchFont` ブランチはコンパイラ網羅性チェックのため残す（実際には到達しない）。
- `demo-05.html` から `EV_FETCH_FONT`・`FONT_URL_MAP`・ハンドラを削除。アプリコードはフォントを意識しない。
- ADR-0042 の「アダプタが URL を持つ」部分をこの ADR に移動。

## URL フォーマット注記

fontique/skrifa は WOFF2 を解釈しない（magic bytes `wOF2` 未対応）。`builtin_font_url` に登録するのは TTF/OTF URL のみ。現在のエントリ：

| family | スクリプト | ofl dir | 形式 |
|--------|---------|---------|------|
| Noto Sans JP | 日本語・CJK共通 | `notosansjp` | TTF variable `[wght]` |
| Noto Sans KR | 韓国語 | `notosanskr` | TTF variable `[wght]` |
| Noto Sans SC | 簡体字中国語 | `notosanssc` | TTF variable `[wght]` |
| Noto Sans TC | 繁体字中国語 | `notosanstc` | TTF variable `[wght]` |
| Noto Sans Arabic | アラビア語 | `notosansarabic` | TTF variable `[wdth,wght]` |
| Noto Sans Thai | タイ語 | `notosansthai` | TTF variable `[wdth,wght]` |
| Noto Sans Devanagari | デーヴァナーガリー（ヒンディー語等） | `notosansdevanagari` | TTF variable `[wdth,wght]` |
| Noto Sans Hebrew | ヘブライ語 | `notosanshebrew` | TTF variable `[wdth,wght]` |

※ `%5B`/`%5D` は `[`/`]`、`%2C` は `,` の URL エンコード。

※ `%5B`/`%5D` は `[`/`]` の URL エンコード（ファイル名 `NotoSansJP[wght].ttf`）。

## 将来の Native Adapter

Native Adapter は `FetchFont { family }` を受け取ったとき：
1. OS のフォントコレクション（fontique native backend）に問い合わせる
2. あれば `register_font_bytes()` で登録
3. なければローカルバンドルまたは独自ソースから取得

Core・アプリは変更不要。
