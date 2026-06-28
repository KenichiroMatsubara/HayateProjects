# km_arrange — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

## 「PRs as a request surface」設問を採用しない

上流は、GitHub/GitLab を選んだ場合に「外部 PR もトリアージ対象に含めるか」を尋ねる設問を追加したが、**このリポジトリは外部 PR をトリアージ対象にしない**方針（`triage/km_arrange.md` 参照）のため、この設問は採用しない。ローカル版（PR 設問なし）を保持する。上流更新時、PR 関連の設問・記述は**取り込まない**。

## description はリポジトリ独自（自動発火トリガー強化）

`description` フロントマターは、Claude Code の自動発火が効くようトリガー語を厚くした独自版を保持する。

## このスキルは vendoring 済みスキルを直接更新しない／自身も直接更新されない

- `setup-matt-pocock-skills` は repo 設定（issue tracker・triage ラベル・domain docs）専用であり、**他の vendoring 済みスキルの本体や `km_arrange.md` を書き換えてはならない**。
- このスキル自身（`pinnedLocal`）の本体更新も、直接行わない。上流からの取り込みは **`/update-matt-pocock-skills` のレビュー関門（差分→リスクスキャン→ユーザ承認）を通した場合に限る**（ADR-0005）。
