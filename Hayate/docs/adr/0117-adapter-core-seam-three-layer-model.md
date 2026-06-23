# アダプター層を Core / Family Adapter / leaf の三層にし、platform-free な共通 seam を Core 所有、family 統一 capability を Family Adapter 所有とする

status: accepted

## Context

これまでアダプターは `adapters/{web,android,ios}` のフラット構成で、各 leaf が薄い直バインディング（ADR-0087/0114）だった。一方で「共通を Core 所有」の流れが進み、scroll 物理（ADR-0113）・edit model（ADR-0069）・Render Host/Font（ADR-0068）が Core / 共有層へ寄ってきた。

二つの未整理が残っていた。

1. **platform-free なのに重複している seam**: `surface_lifecycle`（android 216 行 / ios 238 行・「同型」）・`touch_input`（80 / 86 行・「同型」）・IME 増分入力モデル（`ImeCommand`/`ImeBuffer`/`apply_command`、ios adapter に残存）。いずれも芯は platform-free で、各 leaf に複製されている。ios の `ime_input.rs` は増分入力の Core 化を「別ブランチの再構築で扱う」と明記しており、本決定がその再構築にあたる。
2. **capability の置き場が無い**: mobile/desktop で audio 等のネイティブ API を family 統一で供給したいが、ADR-0014 は Platform Adapter の責務を IME / clipboard / raw 入力の三つに**閉じて**おり、audio 等の置き場が定義されていない。

## Decision

アダプター層を三層に分け、責務を性質で振り分ける。

- **Core（platform-free な共通ロジック）**: platform コードを一切含まない純粋ロジックを所有する。本決定で以下を leaf 重複から Core へ hoist する。
  - **surface 状態機械**: `InitWindow` / `TerminateWindow` / `WindowResized` / `Destroy` の 4 論理イベント → surface ops。leaf は native イベント→4 イベントの glue のみ。
  - **touch 変換**: native タッチ enum を畳んだ `TouchAction` → 座標ベース pointer dispatch の fold。gesture 認識・物理は ADR-0113 で既に Core。leaf は native enum→`TouchAction` の写像のみ。
  - **IME 増分**: `ImeCommand` / `ImeBuffer` / `apply_command`（増分入力モデル）。Core が**両入力モデル（Android = 絶対状態 diff / iOS = 増分 command）を所有**し、両者が共通の `ime_reconcile`（出力、既に Core）へ合流する。leaf は native callback→`ImeCommand`（iOS）/ native buffer→絶対状態（Android）の glue のみ。
- **Family Adapter（`platform/{mobile,desktop}/`）**: 複数 leaf を束ね、**family 内で統一できる platform-bound capability（audio 等）を単一 facade で上位へ供給する**。ビルド時 `cfg(target_os)` で片方の leaf 実装をリンクする facade（ランタイム dispatch ではない）。`web` は family of 1 のため Family Adapter を持たず leaf を直接置く。
- **leaf（`platform/{mobile,desktop}/<platform>` ・ `platform/web/`）**: surface 生成 glue・raw event 配線・`ImeBridge` 実装・capability の platform 実装といった**完全に platform 固有な glue だけ**。アダプタ間で windowing/event-loop glue は共有しない（ADR-0087/0114 を維持）。

**capability の契約は常に Core 所有**（`ImeBridge`/`Surface`/`FontFetcher` と同型・ADR-0068/0069）。`platform/{common,mobile,desktop}/` ディレクトリは実装と family facade の置き場であり、契約の正本ではない。共通 API への昇格は原則 2 実装が揃ってから。ただし **desktop の枠（ディレクトリ + grouping doctrine）は前払いで今作る**（Flutter/RN の prior art で variation 確定済み、ADR-0012 で desktop は確定ターゲット、ADR-0068 の前払い条件を満たす）。個々の capability trait は実装時に Core へ足す（空 trait を先置きしない）。今 trait を切る capability は mobile audio（android+ios の 2 実装が確定）。

これは **ADR-0014 の「責務は三つ」の閉じたリストを reopen** し、capability（audio 等）を leaf / Family Adapter の責務クラスとして追加する。IME/clipboard/raw 入力の plumbing が leaf に残る点は不変。

## Considered Options

- **フラット維持 + 共通は都度 Core 化**: family 統一 capability の置き場が無いまま。audio の統一 facade を提供できず、desktop の grouping doctrine も曖昧なまま残る。却下。
- **Family Adapter を共有 windowing crate にする**: ADR-0087/0114 の「アダプタ間で windowing/event-loop を共有しない」と衝突。却下（Family Adapter が共有するのは capability であって windowing ではない）。
- **capability 契約を adapter 側 commonAPI crate が所有**: 「共通を Core 所有」方針が割れ、`ImeBridge`/`Surface`/`FontFetcher` と別パターンになる。却下。
- **三層 + 契約は Core + family facade は cfg 切替（採用）**: 共通の正本が Core に一本化され、family 統一 facade が成立し、leaf は薄いまま。

## Consequences

- `adapters/{web,android,ios}` → `platform/{web, mobile/{android,ios}, desktop/}` へ再編。`platform/{common,mobile,desktop}/` は capability 実装と family facade の置き場。
- `surface_lifecycle` / `touch_input` の android/ios 重複と iOS の IME 増分入力モデルが Core へ移り、leaf は glue だけに痩せる。
- 新 crate `hayate-adapter-mobile`（cfg facade）。`desktop` は枠のみ（leaf 0、capability trait 未定義）。
- ADR-0014 を reopen（責務に capability を追加）。ADR-0068/0069/0113 の Core-所有パターンを継続、ADR-0114 が「別ブランチで」と予告した IME 増分の Core 化を実行。ADR-0087/0114（windowing 非共有）・ADR-0012（等階級）は維持。
- 受容するリスク: desktop の grouping doctrine を leaf 0 の段階で引くため、最初の desktop leaf 着手時に taxonomy 調整がありうる。capability trait を前置きせず実装時に足すことで露出を抑える。
