> **Superseded by ADR-0049** — WIT は廃止。`@torimi/hayate-protocol-spec`（`proto/spec/*.json`）が機械可読な単一正本となった。

# Raw Layer は WIT world から一時退避し、実装完成まで非公開とする

## Context

Hayate の WIT は Element Layer と Raw Layer の二層構造を定義している（ADR-0013）。しかし現時点の WIT `raw-layer` インターフェースには `create-rect` と `node-remove` しかなく、spec が列挙する `create-text-run` / `create-image` / `create-clip` / `create-layer` / `create-group` は存在しない。

不完全な API を `world hayate` から export すると、wit-bindgen で生成された他言語 SDK に半完成のバインディングが含まれ、利用者が混乱する。

## Decision

**Raw Layer インターフェースの定義は WIT ファイルに残すが、`world hayate` の export からは除外する。**

```wit
world hayate {
  export element-layer;
  // raw-layer は ADR-0033 が解消されるまで意図的に除外する
}
```

Raw Layer を公開するための条件:
1. `create-rect` / `create-text-run` / `create-image` / `create-clip` / `create-layer` / `create-group` がすべて WIT 定義済み
2. Hayate Core 側に実装が揃っている
3. Raw Layer を直接使うユースケース（ゲーム HUD・Infinite Canvas・カスタム layout engine）が具体化している

## Considered Options

**不完全なまま export し続ける**
- Con: SDK 生成時に未実装 API が含まれる。利用者が呼び出してもパニックまたはデッドコード

**Raw Layer を WIT ファイルごと削除する**
- Con: 二層構造という設計原則（ADR-0013）まで消えたように見える

**インターフェース定義は残し、world export のみ外す（採用）**
- Pro: 設計意図が WIT に残る。実装が揃った時点で world に追加するだけでよい
- Pro: Element Layer の内部実装は引き続き Raw Layer の Rust 型を使う（WIT export とは独立）

## Consequences

- `world hayate` には `export element-layer;` のみ残る
- Raw Layer の Rust 実装（`NodeKind`, `SceneGraph`, `vello_bridge`）は変更しない
- Raw Layer の WIT 公開は別途 ADR で決定する
