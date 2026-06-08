# 全プラットフォームを等階級とし、Web を「最初の実装」と位置づける

> **2026-06-07 追記（native 本体・web 先行の理由）:** 本 ADR の「Web が最初の実装」は **native が本体（primary target）** であることを前提とする。web が先行するのは純粋に**開発速度**の事情である：(1) 描画確認が速くイテレーションが速い、(2) リモートで AI 主導開発する際に **AI 自身がスクショで動作確認**できる、(3) **DOM Mode フォールバックとデザインを比較**して問題を発見しやすい。アーキテクチャ上の優遇が無い（下記）ことに加え、**プラットフォーム非依存の芯**（interaction 状態機械 = ADR-0066、Render Host / Font ロード = ADR-0068）は native を見据えて共有層へ置く。native は仮説ではなく確定した本体ターゲットなので、共有 seam を web 単独時点で引くのは投機ではない（ADR-0068）。

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
