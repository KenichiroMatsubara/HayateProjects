# §0 システム & ドキュメント運用

HayateProjects 全体の構造と、ドキュメント体系の運用規範。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。

---

### SYS-01 — モノレポ構造
**規範文:** HayateProjects は単一 `.git` リポジトリをルートとし、`Hayate/` と `Tsubame/` を並列ディレクトリで管理する。pnpm workspace で両者のパッケージを統一スコープに置く。
**出典:** ROOT ADR-0001
**状況:** ✅ — 単一 `.git`、`Hayate/`+`Tsubame/` 並列、`pnpm-workspace.yaml` に両者登録。`origin` は旧 Hayate repo を継承。
**備考:** [履歴 C-0.1] `Tsubame/docs/adr/0001-independent-repository` の「別リポジトリ」決定はリポジトリ構造として superseded（アーキテクチャ上の分離は SYS-02 で有効）。

### SYS-02 — 一方向依存（Hayate は上位を知らない）
**規範文:** Hayate Core は Tsubame・Hayabusa を知らない。結合点は Hayate が所有する `apply_mutations` / `poll_events` 契約（§10）のみ。モノレポ化は AI/クラウド作業都合であり、アーキテクチャ上の結合点ではない。
**出典:** ROOT ADR-0001, ADR-0053, ADR-0035
**状況:** ✅ — `Hayate/crates/core` に Tsubame/Hayabusa 参照なし。Tsubame は `@hayate/protocol-spec` を片方向で消費。
**備考:** この不変条件が設計書の Hayate / §10 / Tsubame の三分割の根拠。

### SYS-03 — CONTEXT.md は語彙のみ
**規範文:** `CONTEXT.md`（ルート統合）はプロジェクトの glossary に徹し、実装詳細・決定履歴を持たない。判断根拠は ADR、規範はこの設計書に置く。
**出典:** ROOT ADR-0001
**状況:** ✅ — ルート `CONTEXT.md`（約25KB）が Hayate/Hayabusa/Tsubame の語彙を一元定義。
**備考:** 用語追加・鋭利化は grill 中にインラインで `CONTEXT.md` を更新する運用（本設計書とは責務分離）。

---

## 集計
| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 3 | SYS-01〜03 |
