# km_arrange — このリポジトリ独自の編集

> ⚠️ このファイルは Matt Pocock 上流ではなく、**このリポジトリ独自の追加・変更**を記録する。
> 上流更新時もこの内容は保持する。`SKILL.md` の指示と衝突する場合は **このファイルを優先**する。

このリポジトリの `triage` スキルは、上流に対して以下を独自に上書きしている。

> **なぜ本文を上流 pristine に戻せないか（pinnedLocal）：** 現在の上流 `triage` は、grill 工程を `/grilling`＋`/domain-modeling` へ委譲し、外部 PR トリアージを本文全体に織り込む形に作り替えられている。これらサブスキルは未導入で、PR トリアージも不採用のため、本文を上流へ戻すと参照が宙に浮き、不要な記述が大量に入る。したがって `SKILL.md` 本文は**自己完結フォークとして保持**する（`skills-lock.json` の `pinnedLocal: true`）。上流差分は `/update-matt-pocock-skills` が参考提示するのみで、本文は自動置換しない。`/grilling`・`/domain-modeling` を将来 vendoring すれば、クリーン分離へ移行できる。

## 1. state は AFK / 完全人力 の二択（HITL state なし）

- `ready-for-agent` = AFK（人手を介さず実装可能）。
- `ready-for-human` = **完全人力**（AI を一切介さず人間が行う：定数・パラメータのチューニング、好み・設計判断、外部アクセス、手動テスト）。
- **HITL（AI＋人間フィードバックループ）の state は存在しない。** 両方必要に見える issue は、`/to-issues` で AFK スライス（実装完了・マジックナンバーは名前付き定数化・値はプレースホルダ可）＋後続の完全人力チューニングスライスに分割されているべき。されていなければ、混在 issue として単独でトリアージせず、メンテナに分割を促す。

**なぜ：** `to-issues` の分割哲学（AFK か完全人力の二択、HITL 禁止）を triage 側でも一貫させるため。中間の HITL state を作ると、「エージェントが拾えるのか人間がやるのか」が曖昧な issue が queue に溜まり、AFK エージェントが安全に着手できなくなる。

## 2. 外部 PR のトリアージは廃止（issue のみ）

上流は「外部 PR も issue と同じ state machine でトリアージする」機能を持つが、**このリポジトリではそれを採用しない**。トリアージ対象は issue のみ。上流更新で外部 PR 関連の記述が増えても取り込まない。

**なぜ：** このリポジトリは PR を issue と同じトリアージ queue で扱う運用にしていない。PR を混ぜると「外部 PR の定義」「PR か issue かの判定」「diff 検証」等が全工程に分岐として入り込み、本来の issue トリアージが複雑化する。issue だけに絞れば state machine が単純に保てる。

## 3. grill は `/grill-with-docs` を使う

issue を grill する必要がある場合、上流の `/grilling` ＋ `/domain-modeling`（このリポジトリ未導入）ではなく、自己完結の **`/grill-with-docs`** セッションを使う。

**なぜ：** `/grilling`・`/domain-modeling` は未導入なので呼んでも壊れる。`/grill-with-docs` が同等（grill＋CONTEXT/ADR 更新）を自己完結で提供する。
