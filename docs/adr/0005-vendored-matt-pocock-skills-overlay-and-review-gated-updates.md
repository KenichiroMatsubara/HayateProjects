# Matt Pocock スキルの vendoring：オーバーレイ方式のチューニングとレビュー関門付き更新

**Status: accepted**

**Date: 2026-06-28**

## Context

このリポジトリは Matt Pocock の公開スキル集（`mattpocock/skills`）を `.claude/skills/` と
`.agents/skills/` に **vendoring**（実体コピー）して使っている。`skills-lock.json` が上流の
ソース・パス・ハッシュを記録する。

二つの問題があった。

1. **更新の安全性。** SKILL.md は Bash・ファイル書き込み・git push・ネットワークを持つ
   エージェントへ注入される「指示」である。上流が悪意ある変更を入れた（あるいは上流アカ
   ウント/リポジトリが侵害された）まま無批判に取り込むと、秘密情報の流出や不正なコード
   push を誘導される **prompt-injection / サプライチェーン攻撃**の経路になる。当時、更新は
   仕組み化されておらず、「最終更新からどう変わったかを見て、危険でなさそうなら取り込む」を
   安全に回す手順がなかった。

2. **独自チューニングの保全。** 複数のスキル（`tdd`・`to-issues`・`triage` 等）に、この
   リポジトリ固有の仕様（AFK / 完全人力 のスライス分類、native sub-issue による親子 issue
   = ADR-0003、`status:*` 進捗マーカー、`AskUserQuestion` ツール不使用 等）が **SKILL.md に
   インラインで直接編集**されていた。base（取り込み時点の無改変上流）が保存されておらず、
   `skills-lock.json` もハッシュしか持たないため、上流を素朴に取り込むと独自編集を**黙って
   上書き**してしまう。どれが独自編集か機械的に復元する術もなかった。

加えて調査の結果、上流は vendoring 時点から大きく変化していた：`caveman`・`zoom-out` は
削除、`diagnose`→`diagnosing-bugs`・`write-a-skill`→`writing-great-skills` に改名、さらに
`grill-me`・`grill-with-docs`・`triage`・`improve-codebase-architecture` は本体を
`/grilling`・`/domain-modeling`・`/codebase-design` というサブスキルへ**委譲する形に作り替え**
られていた。これらサブスキルは未導入のため、上流を丸ごと取り込むと参照が宙に浮いて壊れる。
「全部取り込む」は悪意以前に安全でない。

## Decision

### 1. オーバーレイ方式で独自チューニングを明示する

独自編集を持つスキルには、同ディレクトリに **`km_arrange.md`**（このリポジトリ独自の追加・
変更を記録するファイル）を置く。SKILL.md の冒頭には標準の**参照ブロック**を入れ、「本文を
読む前に `km_arrange.md` を読み、**衝突時は `km_arrange.md` を優先**せよ」と明記する。これに
より、何が上流由来で何がリポジトリ独自かが一目で分かり、更新時に独自部分を保護できる。

ユーザが新たに独自仕様を足すときも `km_arrange.md` に書く。**その追加が元スキルの設計思想と
衝突する場合は、書き込む前に必ずユーザへ確認する。**

### 2. 自動更新しない。レビュー関門付きの更新スキルを設ける

スキルは固定 commit（`skills-lock.json` の `sourceCommit`）に pin し、**自動更新は行わない**。
更新は `/update-matt-pocock-skills`（user-invoked）が担い、次を行う：上流を pin した commit で
取得 → `base → 新上流`の差分 → **全差分にセキュリティ・リスクスキャン**（シェル実行・ネット
ワーク送信・秘密情報読み出し・`git push`/force・削除・難読化・注入文・`description`/トリガー
語の変更）→ 危険印を付けて提示 → **更新1回ごとにユーザの最終 OK で適用**。`km_arrange.md` と
ローカル frontmatter は保持する。**完全無言の自動適用はしない。**

### 3. 分離可能なものは本文を上流 pristine に戻し、不可能なものは `pinnedLocal` で固定

独自チューニングは原則 `km_arrange.md` に分離し、`SKILL.md` 本文は上流 pristine に保って更新
容易性を確保する。ただし分離の質は上流の構造に依存して二分される：

- **クリーン分離（`km_arrange` のみ、`pinnedLocal` なし）**：`tdd`・`to-issues`・`to-prd`。
  本文を pin した上流 SKILL 本文に戻し（frontmatter の `description` だけローカル保持）、独自
  仕様は全て `km_arrange.md` に置く（追加分、および本文の特定節を上書きする分を明示）。本文が
  上流とクリーンに diff できるため、以後の更新は素直。
- **自己完結フォーク（`km_arrange` ＋ `pinnedLocal`）**：`grill-me`・`grill-with-docs`・
  `improve-codebase-architecture`・`setup-matt-pocock-skills`。前3者は上流が本体を
  サブスキル（`/grilling`・`/domain-modeling`）へ委譲する形に作り替えたもの。`setup-matt-pocock-skills`
  は差分が「上流が追加した PR 設問の削除」という removal 型で、かつ同梱 seed（`domain.md` 等）が
  リポジトリ独自に改変済みのため、追加型オーバーレイにクリーン分離できない。これらは本文を自己
  完結フォークとして保持し、上流本体で自動置換しない。

`triage` は当初リポジトリ独自チューニング（外部 PR 不採用・AFK/完全人力 state）と判断したが、
ユーザに確認したところ意図的に入れた記憶が無く、調査でも provenance を特定できなかった（古い
上流の名残の可能性が高い）。ユーザ判断により **`triage` は上流 Matt Pocock 版をそのまま採用**
（km_arrange・`pinnedLocal` を撤去し、純粋 vendoring に戻した）。上流 `triage` は `/grilling`・
`/domain-modeling` を参照するため、ユーザ判断により**両スキルも上流から vendoring 済み**。
`grilling` には環境制約（`AskUserQuestion`/structured-question ツール禁止＝この環境のアプリバグ
で回答済みの質問が再表示されるため）を `km_arrange.md` で付与し、grill が走る全経路でこの制約が
効くようにした。`/grilling`・`/domain-modeling` が導入されたことで、委譲型フォーク3者は将来クリーン
分離へ移行可能になった（当面は自己完結を維持、移行は依頼ベース）。

### 4. 自作スキルは管理対象外

`inherit-prompt`・`make-pr`・`search-issue`・`update-matt-pocock-skills` は Matt Pocock 由来
でないため、`skills-lock.json` に載せず、更新スキルも一切触らない。

## Consequences

- **良い点：** 更新が人間のレビュー関門を必ず通る。独自チューニングが更新で消えない。何が
  独自編集かが `km_arrange.md` に明示され、将来の読み手にも追跡可能。pin により再現性がある。
- **コスト：** チューニング済みスキル（`pinnedLocal`）は上流本体を自動追従しないため、上流の
  改善を取り込むには `/update-matt-pocock-skills` 上で手動レビューが要る。レビューは LLM に
  よる差分読解であり、有効だが完全ではない（巧妙に難読化された悪意はすり抜け得る）。これは
  受容するリスクであり、`pinnedLocal` と km_arrange の明示で被害面を最小化する。
- **却下した代替案：** (a) 自動更新 — レビュー関門が消え、サプライチェーン risk を直に負う
  ため却下。(b) 独自チューニングをインラインのまま base スナップショットで 3-way マージ —
  base が未保存で、上流が委譲化リファクタを行ったため機械マージが破綻する。オーバーレイ＋
  `pinnedLocal` の方が、現状の散らかったインライン編集から無理なく移行できる。
