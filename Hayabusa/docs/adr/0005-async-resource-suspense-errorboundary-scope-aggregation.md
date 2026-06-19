# 非同期・Resource・Suspense・ErrorBoundary を Scope 階層で集約する

status: accepted

## async は script 所有、結果は signal 再入

I/O は副作用であり script の領分（ADR-0001 の責務線）。async 本体は script が起動し、
結果は ABI（ADR-0002）の signal write でリアクティブグラフに**再入**する。

ランタイムコアに async という概念は持たせない。必要なのは
「**外部／async からの signal write が新しい flush を起こす**」スケジューリングフック
1点だけ（flush 合体・ADR-0003 の自然な延長。async 解決＝1つのイベント＝1 flush）。

## Resource は runtime 認識の loading-aware 値

Resource（loading / error / data）は専用機構ではなく、**閉じた値モデルの一員としての
loading-aware 値**である（ADR-0003）。script の async 本体が `loading=true → await →
data / error` と倒す。runtime が loading 状態を認識することで Suspense 集約を自動化できる。

## Suspense はランタイム所有

- loading 中の Resource を境界配下（Scope）で read すると、ランタイムが
  最寄り Suspense 境界の pending を**自動集約**（++ / --）する
- pending > 0 で fallback、0 で content を**構造 reconcile**で切り替える
- 集約の対象は要素幾何ではなく **Scope 階層**

## ErrorBoundary はランタイム所有・Suspense と対称

- throw / 構造エラーを最寄り ErrorBoundary まで巻き戻し、失敗した Scope を teardown して
  fallback を描画する
- 発生源2系統：binding 式評価エラーは**ランタイムが直接 catch**、handler / effect の throw は
  **ABI 越しに error として渡り**ランタイムが最寄り境界へルーティングする
- **データエラー（`Resource.error`）は別チャネル**で reactive に扱う（巻き戻し対象ではない）
- 集約は Suspense と同じく **Scope 階層**基準

loading（Suspense）と error（ErrorBoundary）を「Scope 階層で集約する」一対として揃える。

## Considered Options

- 各言語 SDK が try/catch・loading 集約を実装：言語ごと重複し、subtree 集約を失う。却下。
- ランタイム所有・Scope 集約（採用）：言語横断でゼロコスト。

## Consequences

- async / Suspense / Error がいずれも Scope 階層という単一の基準に乗る
- Resource を値モデルの一員にしたことで suspense 集約が script コールバックなしで回る
- 実装は段階的でよい（初期は手動 `:if resource.loading` でも動く）が、モデルはランタイム所有
