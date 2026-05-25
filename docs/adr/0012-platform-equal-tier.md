# 全プラットフォームを等階級とし、Web を「最初の実装」と位置づける

Hayate は「Web SPA 最優先」ではなく「Web が最初の実装、全プラットフォームが品質で等階級」という原則を採用する。

実装順序は Web が最初である。しかし「Web が特別」ではなく「Web の次に他のプラットフォームが来る」という意味でしかない。

- **実装順序**: Web → macOS / Windows / Linux → iOS / Android
- **アーキテクチャ上の優遇**: なし。Core は Platform Adapter を知らず、wgpu が GPU surface の差を吸収し、WIT が言語・プラットフォーム非依存の契約を定義する

## なぜ区別するか

「Web 最優先」と設計書に書くと、後からネイティブを追加するとき API や IME 実装に Web 固有の前提が残り歪みが生じる。「Web が最初の実装」と書くと、最初から Platform Adapter の境界を正しく引く動機が生まれる。

Flutter Web が Dart と密結合し、ネイティブ優先設計を Web に後付けした歴史的失敗を繰り返さないための決断。

## Consequences

- Platform Adapter は Web・ネイティブそれぞれで独立した一級実装を持つ
- IME インターフェースは WIT で定義し、Web（invisible textarea）とネイティブ（TSM / TSF / IBus 等）が別実装を持つ
- Core に Web 固有の型・概念を混入しない
