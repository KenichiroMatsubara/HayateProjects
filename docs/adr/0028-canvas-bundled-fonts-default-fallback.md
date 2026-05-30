# Canvas モードでのフォントバンドルとデフォルトフォントフォールバック

Canvas モード（WebGPU + Vello + WASM）では fontique のシステムフォント自動発見が無効になる。WebAssembly バックエンドはシステムフォントを一切列挙しないため、`FontContext::new()` は登録済みファミリーゼロの状態で起動する。フォントを明示的に登録しない場合、Parley はグリフを生成できずテキストが不可視になる。

## 決定

- `Noto Sans JP`（Variable, OFL, ラテン + 日本語 + 漢字カバレッジ）をデフォルトフォントとし、`include_bytes!` でバイナリに埋め込む
  - 旧設定: Latin 専用の `Noto Sans`（NotoSans-Regular.ttf, 2 MB）
  - 新設定: `NotoSansJP.ttf`（9.2 MB TTF, ラテン・ひらがな・カタカナ・漢字を網羅）
  - 旧設定の Latin 専用フォントはデモ 5 の日本語 IME 入力で □（tofu）が出る根本原因だった
- `ElementTree::new()` 内でバンドルフォントを登録し、`GenericFamily::SansSerif` にもマップする
- `DEFAULT_FONT_FAMILY = "Noto Sans"` をコア定数として公開する
- `build_text_layout` はリクエストフォントとデフォルトの CSS フォントスタック `"<requested>, Noto Sans"` を構築する
- 追加フォントは `load_font_from_url` による遅延ロードも可能。その際 `register_font` は指定名に加えて `DEFAULT_FONT_FAMILY` にも自動登録するため、`element_set_font_family` なしで全要素から参照される
- `fetch_bytes` は HTTP レスポンスのステータスを検証し、非 2xx の場合は `Err` を返す（旧実装は 404 のレスポンスボディをフォントバイト列として渡していたため fontique が黙って失敗していた）

## 却下した代替案

- **デフォルトフォントも遅延ロード**: `ElementTree::new()` が同期的な設計では、ロード完了前の最初の `render()` でテキストが不可視になる。却下
- **adapter 側での登録**: adapter ごとに重複実装が発生し、漏れが生じる。core の `FontContext` はcore で完結させるべきであるため却下
- **Latin 専用 Noto Sans を維持して日本語は `load_font_from_url` で補う**: `fetch_bytes` のステータス未検証バグ・フォントファミリ名の不一致（"Noto Sans JP" vs デフォルトスタック "Noto Sans"）の二重障害があり、動的ロードが機能しないまま放置されていた。根本修正として差し替えを採用

## バイナリサイズへの影響

NotoSansJP.ttf は 9.2 MB。wasm-pack + brotli 圧縮後はおよそ 3–4 MB の WASM バイナリ増加となる（旧 Noto Sans は圧縮後 500–700 KB）。日本語 IME をデモの中心機能として掲げている以上、このコストは妥当と判断した。

## 将来の変更

バイナリサイズが問題になった場合、`pyftsubset` によるフォントサブセット化（Basic Latin + Hiragana + Katakana + 常用漢字のみ抽出で推定 1–2 MB）を別 ADR で検討する。
