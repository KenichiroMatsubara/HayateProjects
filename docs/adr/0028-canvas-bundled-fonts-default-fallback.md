# Canvas モードでのフォントバンドルとデフォルトフォントフォールバック

Canvas モード（WebGPU + Vello + WASM）では fontique のシステムフォント自動発見が無効になる。WebAssembly バックエンドはシステムフォントを一切列挙しないため、`FontContext::new()` は登録済みファミリーゼロの状態で起動する。フォントを明示的に登録しない場合、Parley はグリフを生成できずテキストが不可視になる。

## 決定

- `Noto Sans`（Variable, OFL）をデフォルトフォントとし、`include_bytes!` でバイナリに埋め込む
- `ElementTree::new()` 内でバンドルフォントを登録し、`GenericFamily::SansSerif` にもマップする
- `DEFAULT_FONT_FAMILY = "Noto Sans"` をコア定数として公開する
- `build_text_layout` はリクエストフォントとデフォルトの CSS フォントスタック `"<requested>, Noto Sans"` を構築する。Parley の `FontFamily::Source` は `parse_css_list` で左から順に解決し、未登録名を黙って無視するため、未知のフォント名を指定しても自動的にデフォルトへ落ちる
- 追加フォント（Noto Sans JP など）は `load_font_from_url` による遅延ロードとし、バイナリには埋め込まない
- WIT `style-prop` に `font-family(string)` バリアントを追加し、将来の WIT-native adapter に対応する

## 却下した代替案

- **デフォルトフォントも遅延ロード**: `ElementTree::new()` が同期的な設計では、ロード完了前の最初の `render()` でテキストが不可視になる。Hayabusa の描画ループに async 待ちを強制するため却下
- **adapter 側での登録**: adapter ごとに重複実装が発生し、WIT-native adapter や native adapter でフォント登録が漏れる。core の `FontContext` はcore で完結させるべきであるため却下

## バイナリサイズへの影響

Noto Sans Variable TTF は約 2 MB。wasm-pack + brotli 圧縮後は約 500–700 KB の WASM バイナリ増加となる。和文・等幅フォントはサイズが大きく（Noto Sans JP は約 4 MB）、`load_font_from_url` による遅延ロードで対応する。

## 将来の変更

バイナリサイズが問題になった場合、`pyftsubset` によるフォントサブセット化（Basic Latin のみ抽出で約 50–100 KB）や、デフォルトフォントもサブセット付き遅延ロードへの移行を別 ADR で検討する。
