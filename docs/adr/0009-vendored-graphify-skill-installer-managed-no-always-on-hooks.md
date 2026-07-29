# graphify スキルの導入：インストーラ管理の vendoring、常時 hook は入れない

**Status: accepted**

**Date: 2026-07-29**

## Context

コードベースに関する質問（アーキテクチャ、ファイル間の関係、どこで何が呼ばれているか）に
答えるたびに、エージェントは grep とファイル読みでコンテキストを大量に消費する。
`graphify`（[Graphify-Labs/graphify](https://github.com/safishamsi/graphify)、PyPI パッケージ名
`graphifyy`）は、リポジトリを**ローカルの決定的な AST 解析**でナレッジグラフ化し、
`graphify query` / `path` / `explain` で**スコープを絞ったサブグラフ**だけを返すスキルである。
このリポジトリへの導入にあたり、次を決める必要があった。

1. **配置と更新経路。** 既存の vendoring 規約（ADR-0005）は `mattpocock/skills` 専用に作られて
   おり、`skills-lock.json` と `/update-matt-pocock-skills` は上流を `mattpocock/skills` のツリー
   として解決する。graphify を同じ lock に載せると、更新スキルが「上流から消滅した」と誤判定する。
2. **常時 hook を入れるか。** 上流のインストーラは既定で `.claude/settings.json` に PreToolUse
   hook（`Bash|Grep` と `Read|Glob` を横取りして `graphify hook-guard` を呼ぶ）を登録し、
   ルートに `CLAUDE.md`、`.claude/CLAUDE.md` を生成する。
3. **サプライチェーン上の扱い。** SKILL.md は Bash とファイル書き込みを持つエージェントへ注入
   される「指示」であり、ADR-0005 が Matt Pocock スキルに課したのと同じリスクを負う。

## Decision

### 1. インストーラ管理の vendoring。手書きしない

スキル本体は上流の公式インストーラで両ツリーへ入れる（プロジェクトスコープ）。

```bash
uv tool install graphifyy
graphify install --project --platform claude    # -> .claude/skills/graphify/
graphify install --project --platform agents    # -> .agents/skills/graphify/
```

`.claude` 側と `.agents` 側は上流が用意した**プラットフォーム別の変種**であり、意図的に完全一致
ではない（subagent 起動が `Agent` ツールか `Task` ツールか、常時ブロックの宛先が `CLAUDE.md` か
`AGENTS.md` か）。ここは Matt Pocock スキルの「両ツリー同一コピー」規約の例外とする。
バージョンは各スキルディレクトリの `.graphify_version` が記録する（導入時 `0.9.30`）。

更新は `uv tool install --upgrade graphifyy` の後に上記 2 コマンドを再実行し、**commit 前に
`git diff` を読んでから**取り込む。ADR-0005 のリスクスキャン観点（シェル実行・ネットワーク送信・
秘密情報の読み出し・`git push`/force・削除・難読化・注入文・`description`/トリガー語の変更）は
そのまま適用する。**SKILL.md を手で編集しない**（次の再インストールで消えるため）。リポジトリ
固有の運用ルールは `AGENTS.md` の graphify 節に置く。

### 2. `skills-lock.json` には載せない

graphify は Matt Pocock 由来ではないため `skills-lock.json` に載せず、
`/update-matt-pocock-skills` は一切触らない（`inherit-prompt` などの自作スキルと同じ扱い）。
provenance は「上流インストーラ ＋ `.graphify_version` ＋ 本 ADR」で担保する。

### 3. 常時 hook と生成 `CLAUDE.md` は採用しない

インストーラが生成した `.claude/settings.json`・ルート `CLAUDE.md`・`.claude/CLAUDE.md` は削除した。

- **PreToolUse hook を入れない。** 生成される hook はインストール時の**絶対パス**
  （例 `/root/.local/bin/graphify`）を埋め込むため、他の開発者のマシンでは解決できない。
  さらに graphify CLI は各自が入れる前提の任意ツールなので、未インストールの環境では
  **全 Bash / Read / Grep / Glob 呼び出しごとに hook が失敗**する。リポジトリ共有の設定として
  副作用が大きすぎる。必要な人は各自 `graphify install --project --strict` 等でローカルに入れる。
- **常時ブロックの置き場は `AGENTS.md`。** このリポジトリは `CLAUDE.md` を持たず `AGENTS.md`
  を唯一のエージェント指示元にしている（ADR-0005 時点の構成）。生成物を足すと指示元が 3 つに
  割れるため、上流の always-on ブロック相当を `AGENTS.md` の graphify 節に書き下ろした。

### 4. `graphify-out/` は追跡しない

グラフ出力（`graph.json`・`GRAPH_REPORT.md`・HTML・wiki）は再生成可能な build output であり、
`dist/` と同じ扱いで `.gitignore` に入れる。各開発者がローカルで `/graphify .` して作る。

## Consequences

- **良い点：** コードベース質問が grep 総当たりからスコープ付きサブグラフ照会に変わり、
  コンテキスト消費が減る。スキル本体は手書きしないので上流更新が素直に取り込める。
  hook を入れないため、CLI 未導入の開発者・CI でも何も壊れない。
- **コスト：** CLI が各自インストール必須で、入れていない人には `/graphify` が動かない
  （スキルの指示は残るが CLI がない）。また `SKILL.md` の `description` は上流のまま広く
  （「コードベースに関するあらゆる質問」）、コードベース質問で自動発火しやすい。上流の
  トリガー設計を無断で狭めるのは避けたが、コンテキスト消費が気になれば `description` の
  ローカル上書き（ADR-0005 と同じくローカル frontmatter 保持の方針）で絞れる。
- **導入時のリスクレビュー：** vendoring した SKILL.md と `references/*.md` を走査した結果、
  外部への送信・秘密情報の読み出し・`git push`・破壊的削除・難読化・注入文は検出されなかった。
  ネットワークアクセスはユーザが GitHub URL を渡したときの clone のみ。SKILL.md は
  `ANTHROPIC_API_KEY` / `OPENAI_API_KEY` を読まないと明示しており、Gemini API は
  `GEMINI_API_KEY` / `GOOGLE_API_KEY` が**既に設定されている場合のみ**、かつ docs・論文・画像の
  セマンティック抽出に限って使う。コードは AST 解析のみでネットワークに出ない。
  レビューは LLM による読解であり有効だが完全ではない（ADR-0005 と同じ受容リスク）。
- **却下した代替案：** (a) Claude Code プラグイン／グローバルインストール — リポジトリを
  clone しただけでは付いてこず、`AGENTS.md` の運用ルールと足並みが揃わない。
  (b) `skills-lock.json` へ相乗り — `/update-matt-pocock-skills` が上流ツリーに無いスキルとして
  誤判定するため却下。
