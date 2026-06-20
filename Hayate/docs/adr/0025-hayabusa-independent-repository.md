# Hayabusa は独立リポジトリを持ち、hayabusa:runtime WIT をそこで定義する

status: superseded by Hayabusa ADR-0006（モノレポ内 `Hayabusa/` クレートへ。WIT 境界は Hayate ADR-0045 で既に撤廃済み）

Hayabusa のコード・WIT・Script Adapter・コンパイラはすべて
Hayate リポジトリとは独立したリポジトリで管理する。
Hayabusa リポジトリが `hayabusa:runtime` WIT（Signal / Computed / Effect 等）を定義し、
Script Adapter はこの WIT をインポートして利用する。

依存関係の方向：

```
Script Adapter
  → hayabusa:runtime WIT（Hayabusa リポジトリ）
  → hayate:core WIT（Hayate リポジトリ）
```

Hayate リポジトリは Hayabusa の存在を一切知らない。

## Considered Options

- **Hayate リポジトリに同居**: `wit/hayabusa-runtime.wit` を Hayate リポジトリに追加する。
  設計原則「Hayate コアは Hayabusa の存在を知らない」（仕様書 §1.2）と
  物理レベルで矛盾し、Hayate の WIT 変更サイクルに Hayabusa が引きずられる。
- **独立リポジトリ（採用）**: Hayate と Hayabusa の一方向依存が物理レベルで保証される。
  それぞれのリリースサイクルを独立して管理できる。
- **WIT ミラー（両リポジトリに置く）**: 二重管理による乖離リスクが高く却下。

## Consequences

- Hayabusa リポジトリが `hayabusa:runtime` WIT の権威ソースとなる
- Hayate の WIT バージョンアップ時、Hayabusa 側で追従が必要（一方向）
- Hayate リポジトリへの Hayabusa 関連コードの混入を防ぐ明確な境界が生まれる
