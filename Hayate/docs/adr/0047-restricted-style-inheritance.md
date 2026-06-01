# Restricted Style Inheritance in Hayate Core

テキスト系スタイルプロパティ（`color` / `font-size` / `font-family`）に限定した継承を Hayate コアの `scene_build` レイヤーで実装する。`scene_build` の `walk()` に `InheritedStyle` 構造体をトップダウンで渡し、各 element は `explicit_*: Option<T>` を持ち、`None` の場合は親から引き継いだ値を使う。

## Considered Options

**Hayabusa コンパイラ層で解決する案を却下。** 継承はテキスト系全プロパティに横断的に適用される基盤的ふるまいであり、コンパイラ層で解決すると全 Script Adapter（TypeScript / Python / Rust）で重複実装が必要になる。コアに置くことで一箇所で保証できる。

**React Native モデル（Text-inside-Text のみ継承）を却下。** RN の制約は ネイティブ OS コンポーネントをラップする構造上の副作用であり、GPU ネイティブで全描画を自前で行う Hayate に同じ制約を持ち込む理由がない。`view` 等の非テキスト element もツリーを通して継承値を通過・上書きできる Flutter モデルを採用する。

**`element_set_style` 時に子孫へ即時伝播する案を却下。** Retained モードでは親スタイル変更のたびに子孫全走査が発生しコストが高い。`scene_build` の `walk()` は既にツリーを一度走査するため、そこに乗せるのが自然。

## Consequences

- `Visual` の継承対象フィールドを `Option<T>` に変更する（`explicit_color` / `explicit_font_size` / `explicit_font_family`）。
- `ResolvedElement` の同フィールドも `Option<T>` に変更する。HTML Mode アダプタは `None` のとき CSS プロパティを設定せず、ブラウザの CSS 継承に委ねる。
- `StylePropKind` enum を新設し `element_unset_style(id, kinds: &[StylePropKind])` を WIT に追加する。一度設定した継承対象プロパティを「未設定（親から継承）」に戻す手段として必要。
- `opacity` はスタイル継承ではなくサブツリーの GPU コンポジット問題として切り離す（別論点）。
- `InheritedStyle::default()` は `color: BLACK` / `font_size: 16.0` / `font_family: None`（バンドルフォント）を初期値とする。ルートデフォルトの変更はルート element への明示スタイル設定で行い、`ElementTree` 自体はデフォルト値を持たない。
