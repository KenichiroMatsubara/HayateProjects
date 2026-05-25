# Platform Adapter の責務を IME・クリップボード・raw 入力変換の三つに限定する

Platform Adapter が担う責務を以下の三つに限定する。

1. **IME 入力**: composition-start / composition-update / composition-end / commit-text を WIT インターフェース経由で Core に通知する
2. **クリップボード**: プラットフォーム固有のクリップボード API を WIT インターフェース経由で抽象化する
3. **raw 入力イベント変換**: プラットフォーム固有のポインタ・キーボードイベントを Hayate の統一イベント型に変換する

以下は Platform Adapter の責務に含めない。

| 責務 | 担う主体 | 理由 |
|------|----------|------|
| サーフェス生成 | wgpu | wgpu が Web/ネイティブの Canvas・Window 差を吸収する |
| フレームタイミング | wgpu / winit | wgpu の surface 管理と一体 |
| アクセシビリティ報告 | AccessKit（Core 組み込み） | クロスプラットフォームライブラリのため Adapter に委譲不要 |

## Consequences

- Platform Adapter は薄い。各プラットフォームの実装量が最小化される
- Core が Adapter 経由でしかプラットフォームと接触しない、という境界が明確になる
- 新プラットフォームの追加コストが三責務の実装に限定される
