# OVERRIDES — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

このリポジトリの `to-prd` スキルは、上流に対して以下を独自に上書きしている。

## 1. PRD は親 issue（`parent` ラベル＋ native sub-issue / ADR-0003）

公開した PRD は親 issue として扱う。`parent` ラベルを付与（未作成なら作成）し、後で `/to-issues` が分解する子 issue を **native GitHub sub-issue** として登録できるようにする（ADR-0003）。

## 2. 進捗バー・自動クローズ運用／手動クローズ禁止

子 issue が揃うと GitHub 上で「達成された子Issue / 全子Issue」の進捗バーが表示され、全子クローズ時に auto-close workflow が PRD を自動クローズする。**PRD を手でクローズしない。**
