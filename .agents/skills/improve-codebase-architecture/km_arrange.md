# km_arrange — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

## 自己完結版を保持（上流の委譲化を取り込まない）

上流は `improve-codebase-architecture` を、設計語彙を `/codebase-design`、グリルを `/grilling`、ドメイン更新を `/domain-modeling` へ委譲する形に作り替えた。これら3スキルは**現在すべて vendoring 済み**だが、本スキルは当面、語彙の定義やインターフェイス設計手順を本体に内蔵した**自己完結版を維持**する。同梱の `LANGUAGE.md` / `INTERFACE-DESIGN.md` / `HTML-REPORT.md` / `DEEPENING.md` も独自に保持する。上流委譲版への移行は依頼があった場合のみ実施。

**なぜ：** 自己完結なら設計語彙（module/interface/depth/seam 等）と deepening 手順が本体に揃い、提案を一貫した用語で出せる。依存が少なく挙動を1ファイルで把握できる。
