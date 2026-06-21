# 親子Issueはネイティブ sub-issues で表現し、GitHub Actions で親を自動 close する

**Status: accepted**

**Date: 2026-06-21**

## Context

Issue tracker は GitHub Issues であり、PRD/親 Issue を `to-prd`、子 Issue（vertical slice）を `to-issues` スキルで生成している。これまで親子関係は次の3点で表現していた。

- 親に `parent` ラベル
- 子 body の `## Parent #406` というテキスト参照
- 子 body 末尾の `## Blocked by` セクション（`search-issue` が正規表現でパースして「unblocked か」を判定）

ここに3つの不満があった。

1. **実行時に見たい情報（ブロッカー）が body の一番下にある。** AFK エージェントを回す前に人間が見るのは「今これ着手していいか」だけなのに、`## Blocked by` がテンプレ末尾にあって埋もれる。
2. **親が手で閉じ残る。** 子が全部終わっても親（PRD）は誰かが気づいて閉じるまで open のまま残る。
3. **「達成された子/全子」が一覧で分からない。** 進捗（3/5 done）を見るには各子を開いて状態を数えるしかない。

「テキスト参照のまま自前ツールで数える」案も検討したが、`Blocked by #406` 等の別文脈の `#406` を誤検出する、done 数を自前計算する保守コストが残る、という弱点があり却下した。

## Decision

親子 Issue を **GitHub ネイティブの sub-issues**（`/repos/{owner}/{repo}/issues/{n}/sub_issues` API）で表現する。テキストの `## Parent` 参照は廃止する。これにより：

- **進捗表示はGitHubに委ねる。** GitHub が親に「N of M done」プログレスバーと達成数を標準UI（Web/アプリのIssue一覧・親Issue画面）で描画する。「達成された子/全子」表示は自前コード0行で満たされる（不満3）。
- **親の自動 close は GitHub Actions が担う**（不満2）。`issues` の `closed` / `reopened` イベントで発火し、sub-issues API で子を列挙して判定する。
  - 子が1個以上あり、その**全 sub-issue が closed**なら親を close する。理由は問わない（wontfix で閉じた子も「closed」として数える）。
  - sub-issue が**ゼロの親は何もしない**（生まれたてPRDの即 close 事故防止）。
  - 子が **reopen** されたら、親が closed なら **reopen** する（状態の一貫性）。
  - Action 自身の close 操作による無限ループは発火元（`github-actions[bot]`）でガードする。
- **ブロッカーをテンプレ最上部に置く**（不満1）。子テンプレの新しい並びは、最上部に依存 Issue リスト（`Blocked by`、用語は既存維持）、続いて `## What to build` / `## Acceptance criteria`。親リンクは sub-issue 関係が担うため `## Parent` 節は削除。

既存の open な親子は、一度 `gh api` で sub-issue 登録して移行する。closed 済みは放置。

## Consequences

- `to-issues` / `to-prd` スキルを「`## Parent` を書く」から「`gh api ... /sub_issues` で sub-issue 登録する」へ書き換える。子テンプレから `## Parent` 節を削除し、`Blocked by` を最上部へ移す。
- `gh` CLI は sub-issue を一級コマンドで持たないため、スキルと Action は `gh api`（REST `/sub_issues`）を直叩きする。sub-issue 追加は子の**REST id**（issue number ではない）を要求する点に注意。
- 新規 workflow `.github/workflows/auto-close-parent.yml` を追加。`issues: [closed, reopened]` で発火。
- `search-issue` の `Blocked by` パースは最上部に移動しても動くよう、セクション位置に依存しない実装を維持する。
- `parent` ラベルは sub-issue 関係から導出可能になり冗長化するが、既存スキルの当面の互換のため残す（将来 sub-issue 有無で代替する余地あり）。
- 親の close/reopen が人手を介さず動くため、AFK エージェント運用中も PRD が閉じ残らない。反面、子を全部閉じた瞬間に親が自動で消えるので、「親を意図的に open のまま残す」運用はできなくなる。
