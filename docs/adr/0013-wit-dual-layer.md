# Hayate の WIT インターフェースを Element Layer と Raw Layer の二層とする

Hayate の公開 WIT インターフェースは二層からなる。

**Element Layer（上位）**: element tree の作成・CSS 風スタイルの設定・ツリー組み立てを受け付ける。Hayate 内部でレイアウト計算（Taffy）とスタイル解決を行い、Raw Layer コマンドに変換する。Hayabusa および他言語 SDK の標準的な利用層。

**Raw Layer（下位）**: 絶対座標・確定スタイル済みの描画コマンドを直接受け付ける。`create-rect` / `create-text-run` / `create-image` / `create-clip` / `create-layer` 等の型付きコンストラクタで構成される。Element Layer は内部でこの層に変換して使う。

両層とも WIT で外部公開する。Raw Layer を公開することで、レイアウトを自前で計算する高度なユースケース（ゲーム HUD・Infinite Canvas・カスタム layout engine）に対応できる。

## Considered Options

- **Element Layer のみ公開**: シンプルだが Raw Layer への直接アクセスが必要なユースケースを切り捨てる。
- **Raw Layer のみ公開**: レイアウト・スタイル解決を全 SDK 実装者が自前で行う必要が生じ、言語非依存の価値が失われる。
- **二層公開（採用）**: 標準用途は Element Layer、高度用途は Raw Layer。Hayate が共通インフラを提供しつつ逃げ道を確保する。

## Consequences

- WIT ファイルは `element-layer` と `raw-layer` の二つのインターフェースを定義する
- wit-bindgen 生成 SDK は両インターフェースを含む
- Raw Layer の型付きコンストラクタ（`create-rect` 等）は Element Layer の内部実装でもある
