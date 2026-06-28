# km_arrange — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> `SKILL.md` 本文は上流 pristine のまま保ち、独自仕様はすべてこのファイルに置く（更新容易性のため）。本文と衝突する場合は **このファイルを優先**する。

上流本文（`SKILL.md`）の手順を土台にしつつ、このリポジトリでは以下を**上書き・追加**する。

## 1. スライスは「AFK」か「完全人力」の二択（HITL 禁止）― 本文の slice 規定を上書き

本文の「Draft vertical slices」の vertical-slice 規定に**置き換えて**、各スライスを **AFK** か **完全人力** のどちらかに分類する：

- **AFK** — 人手を介さず実装まで完了する。tunable な値は**必ず名前付き定数**に抽出（値はプレースホルダで可）。実装が landすれば「done」。
- **完全人力**（fully-manual）— AI を一切介さず人間が行う：定数値のチューニング、好み・設計判断、手動テスト。
- **HITL（AI 実装＋人間フィードバックループの混在）スライスは禁止。** ループが要りそうな作業は、AFK スライス（実装完了・名前付き定数）＋後続の完全人力スライス（値をチューニング）に**分割**する。AFK を優先し、各完全人力スライスはそれが依存する AFK スライスの後に並べる。

## 2. tunable は必ず名前付き定数（マジックナンバー禁止）

AFK スライスでは、調整対象の値を**インラインのマジックナンバーにしない**。必ず名前付き定数に抽出し、後で人間がチューニングできるようにする。

## 3. 「Quiz the user」に項目追加（本文への追加）

提案スライス一覧の各行に **Type: AFK / 完全人力** を表示。確認観点に次を追加：「AFK/完全人力 の割り当ては正しいか（HITL 無し）」「AFK スライスはマジックナンバーを避け、tunable を名前付き定数にしているか」。

## 4. native sub-issue 登録（ADR-0003）と `parent` ラベル ― 本文の `## Parent` 節を置き換え

元が既存 issue（親）の場合、子ごとに：(1) 親に `parent` ラベルを付与（未作成なら作成）、(2) 子を親の **native GitHub sub-issue** として登録（`gh api repos/{owner}/{repo}/issues/<parent>/sub_issues -F sub_issue_id="$CHILD_REST_ID"`、子の REST `id` が必要。issue 番号ではない）。これが GitHub の「N of M done」進捗バーと親の自動クローズ（ADR-0003）を駆動する。

```sh
gh label create "parent" --color 5319E7 --description "親イシュー — broken into child issues" 2>/dev/null || true
gh issue edit <parent-number> --add-label "parent"
CHILD_ID=$(gh api repos/{owner}/{repo}/issues/<child-number> --jq .id)
gh api repos/{owner}/{repo}/issues/<parent-number>/sub_issues -F sub_issue_id="$CHILD_ID"
```

**本文テンプレートの `## Parent` 節は書かない**（native sub-issue 関係が代替する）。

## 5. `Blocked by` を本文先頭に置く ― 本文テンプレートの `## Blocked by` 節を置き換え

本文テンプレートの末尾 `## Blocked by` 節は使わず、各 issue 本文の**先頭**に次の blockquote を置く：

```
> **Blocked by:** #123, #456
```

ブロッカーが無ければ `> **Blocked by:** None — can start immediately`。実行時に人間が「今すぐ着手できるか」を判断する唯一の手掛かりなので、散文を入れず issue 参照だけにする。**この blockquote は本文の最初の行**であること。

## 6. 親 issue の扱い

親 issue に許す変更は **`parent` ラベル付与と sub-issue 登録のみ**。本文の編集・手動クローズはしない。全子クローズ時に auto-close workflow が親を自動クローズする（ADR-0003）。
