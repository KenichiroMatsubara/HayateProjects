# OVERRIDES — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

## 1. 自己完結版を保持（上流の委譲化を取り込まない）

上流は `grill-with-docs` を「`/grilling` を `/domain-modeling` 付きで実行する」だけの空洞版に作り替えたが、**このリポジトリは `/grilling`・`/domain-modeling` サブスキルを導入していない**ため、自己完結のフル本体（CONTEXT.md / ADR の domain-awareness、用語のシャープ化、ADR を控えめに提案する手順を内蔵）を保持する。上流更新時、この空洞化は**取り込まない**。同梱の `CONTEXT-FORMAT.md` / `ADR-FORMAT.md` もこのリポジトリ独自に保持する。

## 2. 【厳守】質問は平文で。AskUserQuestion / structured-question ツールは使用禁止

グリル中の質問は、必ず**自分のメッセージ内に平文（散文）**で書く。**`AskUserQuestion`（structured-question）ツールはこの環境で不安定なため使用してはならない。** 質問と推奨回答を文章で提示し、ユーザは平文で返す。1問ずつ、各回答を待ってから次へ。
