# render() はタイムスタンプを受け取り、カーソル点滅は Hayate が内部管理する

## Context

TextInput のカーソル点滅（500ms 周期）を誰が管理するかという問題があった。

当初の実装では `tick_cursor(timestamp_ms: f64)` という独立した `#[wasm_bindgen]` 関数を JS の `requestAnimationFrame` から呼び出していた。これは WIT インターフェースの外側にある実装詳細であり、WIT を単一ソースとする原則（ADR-0015）に反する。

## Decision

**`render()` のシグネチャを `render(timestamp-ms: f64) -> ()` に変更し、カーソル点滅を Hayate Core が内部管理する。**

```wit
render: func(timestamp-ms: f64) -> ();
```

上位層（Hayabusa）は毎フレーム `performance.now()` 等の単調増加タイムスタンプを渡すだけでよい。Hayate 内部で前回描画時刻との差分を計算し、500ms 超過でカーソル表示状態をトグルする。

別途 `tick_cursor()` や `element-set-cursor-visible()` の WIT エクスポートは設けない。

## Considered Options

**上位層（Hayabusa）がカーソル点滅を管理する**
```wit
element-set-cursor-visible: func(id: element-id, visible: bool) -> ();
```
- Hayabusa がタイマーを保持し、500ms ごとに `element-set-cursor-visible` を呼ぶ
- Pro: Hayate がタイマーロジックを持たない
- Con: カーソル点滅は TextInput の描画仕様であり Hayate の責務。Hayabusa に実装を強制するのは責務の漏れ。タイマー精度もフレームループの粒度に依存する

**render() にタイムスタンプを渡す（採用）**
- Pro: 上位層は `performance.now()` を渡すだけ。カーソル点滅ロジックは Hayate に閉じる
- Pro: タイムスタンプは将来的にアニメーション補間・スプリング物理・パーティクル等の時間依存描画にも汎用的に使える
- Con: `render()` のシグネチャ変更が必要。既存の呼び出し元はすべて更新が必要

## Consequences

- WIT `render: func(timestamp-ms: f64) -> ()` に変更
- Hayate Core 内部で `last_cursor_toggle_ms: f64` を保持し、`render()` 呼び出しのたびにカーソル点滅状態を更新する
- `tick_cursor()` wasm-bindgen 関数は削除する
- `element-set-cursor-visible()` WIT エクスポートは追加しない
- 上位層は `performance.now()`（Web）または `Instant::now().elapsed().as_millis()`（ネイティブ）等の単調増加タイムスタンプを渡す
