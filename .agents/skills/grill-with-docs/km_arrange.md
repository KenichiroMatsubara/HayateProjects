# km_arrange — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

## 1. 自己完結版を保持（上流の委譲化を取り込まない）

上流は `grill-with-docs` を「`/grilling` を `/domain-modeling` 付きで実行する」だけの空洞版に作り替えた。`/grilling`・`/domain-modeling` は**現在 vendoring 済み**だが、本スキルは当面**自己完結のフル本体**（CONTEXT.md / ADR の domain-awareness、用語のシャープ化、ADR を控えめに提案する手順を内蔵）を維持する。同梱の `CONTEXT-FORMAT.md` / `ADR-FORMAT.md` も独自に保持する。上流委譲版への移行は依頼があった場合のみ実施。

**なぜ：** 自己完結なら grill＋ドキュメント更新の手順を1ファイルで把握・調整できる。移行する場合でも、AskUserQuestion 禁止は `grilling/km_arrange.md` が担うので失われない。

## 2. 【厳守】質問は平文で。AskUserQuestion / structured-question ツールは使用禁止

グリル中の質問は、必ず**自分のメッセージ内に平文（散文）**で書く。**`AskUserQuestion`（structured-question）ツールは使用してはならない。** 質問と推奨回答を文章で提示し、ユーザは平文で返す。1問ずつ、各回答を待ってから次へ。

**なぜ：** 本文に書いてある通り「the structured-question tool is **unreliable in this environment**」だから。具体的には、この環境のアプリ側バグで**一度答えたはずの質問が何度も繰り返し再表示されて邪魔**になる。平文で聞けばこの問題が起きない。
