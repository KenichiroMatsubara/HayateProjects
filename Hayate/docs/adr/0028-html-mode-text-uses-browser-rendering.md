# HTML Mode のテキスト描画はブラウザネイティブレンダリングを使う

## Context

Canvas Mode では Parley（シェイピング・行折り返し）→ Vello（GPU グリフ描画）という自前のテキストパイプラインを使う。
HTML Mode でも同じパイプラインを通した場合、fontique の Wasm バックエンドはシステムフォントを持たないダミー実装（`wasm32-unknown-unknown` ターゲットではフォントが一切存在しない）であるため、フォントを明示的に登録しない限りグリフが一切描画されない。

## Decision

**HTML Mode のテキスト描画はブラウザのネイティブレンダリングに委ねる。**

`element_set_text()` で設定されたテキストは `HtmlElement::set_inner_text()` でそのまま DOM テキストノードとして出力する。フォントサイズ・テキスト色・フォントファミリーは CSS プロパティ（`font-size` / `color` / `font-family`）として設定し、ブラウザのフォントエンジンが描画を担う。Parley・fontique・skrifa は HTML Mode では呼ばれない。

Canvas Mode と HTML Mode でテキストパイプラインが異なることは許容する。HTML Mode は WebGPU が利用できない環境向けのレンダリングバックエンドであり、ブラウザが持つフォントスタックを活用することは自然な選択である。

## Consequences

- HTML Mode のテキスト描画品質（カーニング・ヒンティング・サブピクセルレンダリング等）はブラウザ依存となり、Canvas Mode と完全一致しない
- HTML Mode では `register_font_bytes()` / `load_font_from_url()` で登録したカスタムフォントは CSS `font-family` 経由でしか適用できない。つまりブラウザが当該フォームを認識できる場合（Web Font として事前にロード済み）にのみ機能する
- Canvas Mode は引き続き Parley + Vello の自前パイプラインを使う。Canvas Mode でカスタムフォントを使う場合は必ず `register_font_bytes()` でフォントデータを渡す必要がある
- ADR-0005 の「Linebender テキストスタック採用」は Canvas Mode に限定される決定として読む
