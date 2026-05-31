# IME イベントは Element Layer に届ける

## Context

Platform Adapter が Core に通知する IME イベント（composition-start / composition-update / composition-end / commit-text）を、WIT の二層構造（ADR-0013）のどちらに届けるかを決定する必要がある。

Raw Layer は絶対座標・描画プリミティブを直接受け付ける低レベル層であり、一見すると IME の座標計算を自前で行えるように見えるため、Raw Layer への通知も候補として検討した。

## Decision

IME イベントは **Element Layer に届ける**。

## Considered Options

- **Raw Layer（却下）**: Raw Layer には `text-input` という概念が存在しない。IME 候補窓の位置計算にはフォーカス中の `text-input` フィールドのスクリーン座標が必要であり、それは Taffy レイアウト結果として Element Layer が保持する。Raw Layer にフォーカス管理やレイアウト座標の概念を持ち込むと Raw Layer の抽象が崩れる
- **Element Layer（採用）**: `text-input` は Element Layer の概念。IME 候補窓位置に必要なレイアウト座標も Element Layer が持つ。IME パスが一層に収まり、責務の境界が明確になる

## Consequences

- Raw Layer を直接使うユーザー（Infinite Canvas・ゲーム HUD 等）が IME 付きテキスト入力を実装する場合、Platform Adapter の IME 通知経路を自前で用意するか、Element Layer の `text-input` をラップして使うことになる
- Element Layer は IME の composition 状態（変換中文字列・確定文字列）を保持する責務を持つ
- Platform Adapter は IME イベントを Core に渡す際、Element Layer の WIT エンドポイントを呼び出す
