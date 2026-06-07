# HayateProjects 設計書（正本）

この `docs/spec/` がシステムの**規範の単一正本**である。「何であるべきか」はここに書く。「なぜそう決めたか」は各項目が参照する ADR（`*/docs/adr/`）に残る。語彙は [`CONTEXT.md`](../../CONTEXT.md)。

> **状態:** パイロット構築中。現在フル構築済みは **§10 のみ**。他パートは §10 の形式承認後に量産する。

## 読み方

各パートは規範文（要件）単位の項目で構成される。1項目のスキーマ:

| フィールド | 意味 |
|---|---|
| **ID** | パート接頭辞 + 連番（例 `PROTO-04`）。安定識別子。 |
| **規範文** | 現在形・テスト可能な契約一文。 |
| **出典** | 根拠 ADR（n対多）。supersededなADRは項目化せず履歴として備考に残す。 |
| **状況** | ✅実装済み / 🟡部分 / ⬜未実装。✅🟡は file:line or テスト参照を必須。 |
| **備考** | 矛盾・supersede履歴・Open論点。 |

**状況の定義** — ✅実装済み: 規範文の全要素がコードに存在しテスト or 実体で担保。🟡部分: 中核は在るが規範文の一部（検証層・方向・モード）が欠落。⬜未実装: コードに痕跡なし、または未決。

## パート一覧

| § | パート | 状態 |
|---|---|---|
| §0 | システム & ドキュメント運用 | 未着手 |
| §1 | Hayate Core 原則 | 未着手 |
| §2 | Element Layer | 未着手 |
| §3 | Layout | 未着手 |
| §4 | Raw Layer / Scene Graph / Rendering | 未着手 |
| §5 | Text / Font / IME | 未着手 |
| §6 | Event Model | 未着手 |
| §7 | Scroll | 未着手 |
| §8 | Web Adapter & Modes | 未着手 |
| §9 | Platform Adapter & Accessibility | 未着手 |
| **§10** | **[Protocol & Wire Contract](./10-protocol-wire-contract.md)** | **✅ パイロット構築済み** |
| §11 | Tsubame | 未着手 |
| §12 | Hayabusa【凍結】 | 未着手 |

## 実装ステータス・ダッシュボード

| § | ✅実装済み | 🟡部分 | ⬜未実装 | 計 |
|---|---|---|---|---|
| §10 | 15 | 2 | 2 | 19 |
| **合計（構築済み分）** | **15** | **2** | **2** | **19** |

🟡部分 = `PROTO-09`（生成codecへ移行済だが手書きparser残存疑い）, `PROTO-11`（C3結合がStubモック）。
⬜未実装 = `PROTO-17`（delivery方向の共有fixture欠落）, `PROTO-19`（app font ID接続 未決）。

## 矛盾マップ

種別: **[履歴]** = 機械的supersession/amend、出典ADR本文が勝者を宣言済み → 自動解決。**[衝突]** = 番号衝突等の表記上の問題。**[要判断]** = 真の未解決矛盾・実コードとの食い違い → grill でエスカレーション。

| ID | 種別 | 内容 | 解決 |
|---|---|---|---|
| C-10.1 | 履歴 | WIT を契約正本とする方針（0013/0015/0033/0039）→ JSON spec 正本（0049 supersede） | `PROTO-01`/`PROTO-02` |
| C-10.2 | 履歴 | 正本形式 `protocol.yaml`（0049原案）→ `proto/spec/*.json`（0053 amend）。slug "protocol-yaml" は名残 | `PROTO-01` |
| C-10.3 | 履歴 | `element_create` を batch 外個別呼び出し（0039）→ `OP_CREATE=9` を batch 内（0005 supersede） | `PROTO-06` |
| C-10.4 | 履歴 | 文字列 op は `apply_mutations` 外（0039）→ `texts[]` string table 統合（0052 supersede） | `PROTO-07` |
| C-10.5 | **要判断** | ADR-0055 の検証層 C1–C4 は `apply_mutations` 方向のみ。「event 方向は既に対称」の仮定で delivery 方向に共有 fixture を置いていない。両言語の delivery wire 一致が無保証 | `PROTO-17`（未解決・要エスカレーション） |

## 運用（governance）

- この設計書が規範の正本。実装はここから駆動する。
- 既存 ADR は削除せず「なぜ」の記録として残置。各項目が出典 ADR へリンクする。
- 今後の新決定: まず設計書を更新。grill 基準（不可逆 × 意外 × 真のトレードオフ）を満たす重大決定のみ ADR を追記。
- `decisions-pending.md` / `TODO.md` の内容は本設計書の Open / 未実装項目に吸収し、両ファイルは archive 送りとする（パイロット承認後に実施）。
- `CONTEXT.md` は語彙のみ。実装詳細・決定は書かない。
