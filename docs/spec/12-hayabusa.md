# §12 Hayabusa【凍結 / 将来】

Hayate Element Layer 上の Signal ベース SFC フレームワーク。**現行の開発優先ではない**（ADR-0051）。
本パートの項目は設計確定だが実装は将来。repo 内に Hayabusa 実装コード・`.hybs` ファイルは無い。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。本パートは原則 ⬜（設計確定・実装は将来）。

---

### HAYA-01 — テンプレートは Hayate 要素語彙を使う
**規範文:** Hayabusa テンプレートのタグ名は Hayate の element-kind 語彙（`view`/`text`/`image`/`button`/`text-input`/`scroll-view`）に直接マップし、HTML タグ名は採らない。式は `{}` の制限付き言語非依存 DSL で書く。
**出典:** ADR-0023
**状況:** ⬜（設計確定・実装は将来） — `.hybs` ファイルなし。`CONTEXT.md`「Template DSL」に定義済み。
**備考:** —

### HAYA-02 — Rust フレームワーク + 多言語埋め込み
**規範文:** Hayabusa は Rust クレートとして `hayate-core` に直接依存し（WIT 境界を経由しない）、Signal グラフの実体を Hayabusa Rust コアが保持する。多言語スクリプト層は言語ランタイム埋め込み（TypeScript: QuickJS / Python: PyO3 / Rust: native）で提供する。
**出典:** ADR-0045（ADR-0024 を supersede）, ADR-0035
**状況:** ⬜（設計確定・実装は将来） — Hayabusa クレートなし。
**備考:** [履歴 C-12.1] ADR-0024「Signal ランタイムを WIT 公開・単一 WASM」は ADR-0045 で supersede（言語ランタイム埋め込みで WIT 境界をゼロにするため）。QuickJS/PyO3 戦略は検証フェーズ。

### HAYA-03 — Hayabusa は独立リポジトリ（一方向依存）
**規範文:** Hayabusa は独立リポジトリとして管理し、Hayate コアは Hayabusa の存在を知らない（一方向依存）。
**出典:** ADR-0025
**状況:** ⬜（設計方針） — 現状は HayateProjects モノレポに物理的に並列しうるが、Hayabusa 実装自体が未着手。
**備考:** [履歴 C-12.2] ROOT ADR-0001（モノレポ化）と並立。物理構造はモノレポ、アーキテクチャ一方向依存は ADR-0025 の intent を保持。

### HAYA-04 — SFC 拡張子は .hybs
**規範文:** Hayabusa の SFC ファイル拡張子は `.hybs`（HaYaBuSa）とする。1ファイル = 1コンポーネント（名前はファイル名のアッパーキャメル）。
**出典:** ADR-0026
**状況:** ⬜（設計確定・実装は将来） — `.hybs` ファイルなし。
**備考:** —

### HAYA-05 — Hot Reload は言語別
**規範文:** Hot Reload は `<template>` / `<style>` を全言語で即時反映する。`<script>` は TypeScript・Python では即時反映、Rust はフルリビルド後にリロードする。
**出典:** ADR-0027
**状況:** ⬜（設計確定・実装は将来） — 実装なし。
**備考:** —

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ⬜未実装（設計確定・将来） | 5 | HAYA-01〜05 |

> §12 全体が「設計のみ確定・実装は将来」。徹底実装フェーズの対象外（ADR-0051 により開発優先は Tsubame）。
