# OVERRIDES — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

## 自己完結版を保持（上流の委譲化を取り込まない）

上流は `improve-codebase-architecture` を、設計語彙を `/codebase-design`、グリルを `/grilling`、ドメイン更新を `/domain-modeling` へ委譲する形に作り替えた。**このリポジトリは `/grilling`・`/domain-modeling` を導入していない**（`/codebase-design` は導入済み）ため、語彙の定義やインターフェイス設計手順を本体に内蔵した自己完結版を保持する。

同梱の `LANGUAGE.md` / `INTERFACE-DESIGN.md` / `HTML-REPORT.md` / `DEEPENING.md` もこのリポジトリ独自に保持する。上流更新時、未導入サブスキルへ委譲する変更は**取り込まない**（取り込むと `/grilling` 等への参照が宙に浮く）。
