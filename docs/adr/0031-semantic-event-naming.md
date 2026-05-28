# イベント命名はセマンティック型（物理操作名ではなく意図名）を採用する

## Context

Hayate の `poll-events()` が返すイベントの命名方針として、二つの流派があった。

**物理型（W3C Pointer Events 準拠）**
```
pointer-enter / pointer-leave   ← カーソルが要素に入った・出た
pointer-down / pointer-up       ← ボタン押下・離放
pointer-move                    ← ポインタ移動（座標付き）
```

**セマンティック型（UI 状態遷移名）**
```
hover-enter / hover-leave       ← ホバー状態開始・終了
active-start / active-end       ← アクティブ状態開始・終了
pointer-move                    ← ポインタ移動（座標付き、物理名を維持）
```

Rust 実装は当初 `PointerEnter` / `PointerLeave` / `PointerUp` という物理名を採用していたが、ADR-0019 はセマンティック名（`hover-enter` / `active-start` 等）を仕様として記述していた。この矛盾を解消するため、どちらが正しいかを明示する必要があった。

## Decision

**セマンティック型を正とする。Rust 実装を spec に合わせて改名する。**

| 物理名（旧 Rust） | セマンティック名（WIT・spec） |
|---|---|
| `PointerEnter` | `HoverEnter` / WIT: `hover-enter` |
| `PointerLeave` | `HoverLeave` / WIT: `hover-leave` |
| `PointerDown`（未実装） | `ActiveStart` / WIT: `active-start` |
| `PointerUp` | `ActiveEnd` / WIT: `active-end` |

`pointer-move` は例外的に物理名を維持する。ドラッグ実装で「どこにあるか」が必要であり、hover-enter/leave と異なり target が存在しない（キャンバス全体で追跡）ため、物理的な意味しか持たない。

## Considered Options

**物理型（W3C 準拠）を採用し spec を書き換える**
- Pro: W3C Pointer Events と同名。タッチ・スタイラスへの拡張が容易
- Con: `click` はセマンティックイベントなのに `pointer-down/up` は物理イベントという混在が生まれる。既存イベント群（`click`, `focus`, `blur`, `scroll`）はすべてセマンティック型であり、モデルの一貫性が崩れる

**セマンティック型（採用）**
- Pro: 既存イベント群（`click`, `focus`, `blur`, `scroll`, `text-input`）と命名スタイルが一致。Hayabusa 開発者が `hover-enter` を受け取れば `:hover` スタイル切替と直結できる
- Con: タッチデバイスで "hover" の語が直感的でない。ただしタッチで指が要素上にあることは `hover-enter` として通知することで解決可能（タッチドラッグ中の要素追跡）

## Consequences

- WIT `event` variant: `hover-enter(element-id)`, `hover-leave(element-id)`, `active-start(element-id)`, `active-end(element-id)`, `pointer-move(pointer-move-event)` を追加
- Rust `Event` enum を改名: `PointerEnter → HoverEnter`, `PointerLeave → HoverLeave`, `PointerUp → ActiveEnd`, 新規追加 `ActiveStart`
- `active-end` は座標を持たない。`active-start` と対称にする。ドラッグ終点が必要な場合は `pointer-move` で追跡する
- `modifier-keys` は WIT `flags` 型で定義し、`key-down-event` に含める（u32 ビットマスクより自己記述的）
- `pointer-move` は target を持たない。キャンバス全体の絶対座標 `(x, y)` のみ
- ADR-0019 の内容はこの ADR で補完される。イベント名の列挙はこの ADR が正典
