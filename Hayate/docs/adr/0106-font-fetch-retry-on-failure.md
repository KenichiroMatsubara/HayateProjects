# フォントフェッチ失敗をリトライ可能にする（恒久ラッチの解消）

## Context

ADR-0042 / ADR-0043 のオンデマンドフォント調達は、`.notdef` 検出 → `FetchFont { family }`
→ web アダプタが CDN から取得 → `register_font()` で登録、という経路を取る。重複イベント
抑止のため、Core は要求したファミリを `pending_font_fetches: HashSet<String>` に入れていた。

問題は **削除が成功時（`register_font`）にしか起きない** ことだった。フェッチが失敗すると
アダプタは `console.warn` するだけで pending から外れず、同じファミリの再要求が二度と発行
されない。新規 GitHub Pages デプロイ直後など jsdelivr が一時的に 403/429/瞬断を返すと、
そのファミリはセッション中ずっと豆腐（□）のまま固定された（issue #343）。

## 決定

### 1. 失敗を Core に伝える経路を追加：`ElementTree::font_fetch_failed(family) -> bool`

成功時の `register_font` と対になる失敗報告 API を Core に置く。アダプタは fetch エラー時に
これを呼ぶ。戻り値はリトライ予定（`true`）か断念済み（`false`）か。

### 2. `pending_font_fetches` を `FontFetchTracker` に置き換え

ファミリ単位で 3 状態を持つ：

- **in-flight**：要求済み・結果待ち。重複 `FetchFont` を抑止する（旧 HashSet と同じ役割）。
- **attempts**：失敗回数。`MAX_FETCH_ATTEMPTS`（= 3）に達したら断念。
- **exhausted**：断念済み。二度と要求せず、ログも暴走しない。

`should_request()` は in-flight でも exhausted でもないときだけ `true`。`mark_failed()` は
in-flight を外して失敗回数を進め、予算が残れば `WillRetry`、尽きれば `GaveUp` を返す。
`font_fetch_failed` は `WillRetry` のとき `mark_fonts_dirty()` を立て、次フレームの再シェープで
ギャップが再検出され `FetchFont` が再発行される。成功時の `mark_loaded()` は全状態をクリア
するので、後で同じファミリが再度欠けても新規に要求できる。

### 3. 抑制：Core の有限予算 ＋ アダプタの指数バックオフ

- **有限回数**は Core が握る（`MAX_FETCH_ATTEMPTS`）。これが「同等の抑制」であり、
  恒久的に失敗し続けるファミリの再要求・ログの暴走を止める単一の真実点。
- **タイミング（バックオフ）**は web アダプタが握る。fetch 失敗時、ファミリ別の試行回数から
  `BASE << (n-1)`（上限あり）の遅延を `setTimeout` で待ってから失敗を報告する。報告が遅れる
  間ファミリは in-flight のままなので再要求は発生せず、結果として試行間隔がバックオフ幅で
  空く。Core が断念（`false`）または成功したらアダプタ側のカウンタも破棄する。

責務分割は ADR-0043 と整合：Core は方針（いつ諦めるか）、アダプタはプラットフォーム固有の
タイミング（`setTimeout`）を持つ。

## 却下した代替案

- **アダプタ内でリトライループを完結**：Core が失敗を知らないため pending が解けず、
  ADR-0042 の「成功時のみ削除」問題が残る。issue #343 の主旨に反する。
- **時刻ベースのバックオフを Core に入れる**：`compute`/`settle` に時刻を通す必要があり
  侵襲的。タイマーは元々アダプタ層（`setTimeout`）にあるので、そこへ置くのが自然。

## 影響

- `core/element/font_fetch.rs`（新規）：`FontFetchTracker` と `MAX_FETCH_ATTEMPTS`。
- `core/element/layout_pass.rs`：`pending_font_fetches` を `font_fetches: FontFetchTracker` に
  置換。4 か所の発行サイトを `should_request` / `mark_requested` に更新。WASM 同等の
  テスト用フォント文脈シード（`set_wasm_like_font_context`）を追加。
- `core/element/tree.rs`：`register_font` は `mark_loaded` を呼ぶ。`font_fetch_failed` を追加。
  テスト補助 `test_set_wasm_like_fonts` / `test_element_glyph_ids`。
- `adapters/web/canvas.rs`：`font_failure_queue` / `font_fetch_attempts` を追加。`poll_events`
  の失敗ハンドラがバックオフ後に失敗を積み、`render` が `font_fetch_failed` へドレインする。
- テスト `core/tests/font_fetch_retry.rs`（新規）：初回失敗 → 再要求 → 登録 → 実グリフ描画、
  および恒久失敗の断念（`system_fonts: false`）。
