# text-input 編集を core の EditState に集約し、IME を ImeBridge trait で分離する（候補 D1）

**Status: accepted（ADR-0066/A1 と統合。ADR-0068 の platform trait パターンを踏襲。ADR-0014/0016/0017 を精緻化）**

**Date: 2026-06-07**

## Context

text-input の編集状態は `Element` に（`text_content` / `preedit` / `cursor_byte_index` / `cursor_visible` / `content_layout`）、操作は `ElementTree` に8メソッド（`set_text_content` / `append_text_content` / `backspace` / `set_preedit` / `commit_preedit` / `paste` / `set_cursor_visible` / `get_text_content`）として直付けされている。だが**編集セマンティクスが `hayate-adapter-web` に漏れている**（IME 関連の所在監査より）：

- `on_key_down`（`:480`）が `Backspace`→`element_backspace`、`Enter`→`append_text_content("\n")` と**キー→編集をマッピング**。
- `on_text_input`→`append_text_content`。
- `on_composition_start/update`→`element_set_preedit`、`on_composition_end`→`set_preedit("")`+`append_text_content`。**adapter が commit をインライン再実装し、core の `element_commit_preedit` は未使用**（分岐・乖離リスク）。
- **EditContext API のラップは JS（wasm 境界の上）で ad-hoc**、trait 化されていない。
- **character bounds 供給（候補窓位置）が欠落** — core→IME の逆方向経路が無い。CONTEXT は「候補窓位置に Taffy レイアウトが必要」とするのに未実装。

目標像は既に文書化された設計意図である（CONTEXT「IME イベントは Element Layer に届く・候補窓位置は Taffy」/ ADR-0017「preedit は Element Layer」）。**コードが追いついていない**だけ。native が本体（ADR-0012）である以上、IME も Surface/FontFetcher（ADR-0068）と同じく platform を薄く包むべき。

## Decision

### 1. EditState（hayate-core 内部 seam）

編集状態と操作を **`EditState { text_content, preedit, cursor_byte_index }`** に集約。操作 `insert` / `append` / `backspace`（char 境界）/ `set` / `paste`（preedit を先に commit）/ `set_preedit` / `commit_preedit` / `display_text`。`Element { edit: Option<EditState>, .. }`（TextInput のみ Some）。**編集セマンティクス（キー→編集・commit・入力 append）は core に置く** — A1（ADR-0066）で core へ移る入力ハンドラ（`on_key_down`/`on_text_input`/`on_composition_*`）が `EditState` を呼ぶ。`cursor_visible`（点滅・ADR-0032）と `content_layout`（Parley キャッシュ）は render-side に残す（編集状態ではない）。

### 2. ImeBridge trait（platform seam、ADR-0068 と同型）

adapter は **platform IME を `ImeBridge` trait の裏にラップするだけ**（web: EditContext / native: TSF / TSM / IBus）。core が `ImeBridge` を使い、composition を受け取り（in）、character bounds と text/selection を返す（out）。adapter は **ラップ＋raw 入力翻訳のみ**で、編集の意味を持たない。

### 3. character bounds export（core 新設）

core が `cursor_byte_index` ＋ `content_layout`（Parley）＋ element layout（Taffy）から **cursor rect を計算**し、`ImeBridge` 経由で IME 候補窓位置へ供給する。欠落していた core→IME 経路を埋める。

## Consequences

- adapter は「`ImeBridge` 実装（EditContext ラップ）＋raw 入力翻訳」だけに痩せる。編集セマンティクス・commit・キー→編集マッピングを持たない。
- core の `element_commit_preedit` が唯一の commit になり、adapter のインライン再実装を削除（乖離を消す）。
- `EditState` が単体 test 可能（backspace の char 境界・paste が preedit を先に commit・commit）。tree 全体不要。
- native adapter は `impl ImeBridge`（TSF/TSM/IBus）＋raw 翻訳だけで `EditState`・character bounds を再利用。薄い（native 本体・ADR-0012/0068 と整合）。
- cursor 位置の成長点（クリック配置＝#3 の Parley 点→byte、選択）が `EditState` 1箇所に集まる。
- text-input は IFC では leaf（ADR-0063）なので `EditState` と IFC は衝突しない。content_layout は text-input の表示、IFC とは別経路。

## Considered Options

- **EditState 抽出のみ（ImeBridge / character bounds を別 ADR に）**：#1（trait 無し）・#2（bounds 欠落）が残り「adapter は EditContext だけ」が達成されない。A1 が移す編集トリガの受け皿（IME seam）も未定義。却下。
- **現状維持**：編集セマンティクスが adapter に散在、commit 乖離、候補窓経路なし。
- **統合形（本決定）**：EditState（core）＋ ImeBridge（platform trait）＋ character bounds（core 新設）。IME の意味が全部 core に集まり、adapter は EditContext ラップだけになる。

## 関係

- ADR-0066（A1）：入力ハンドラの core 移管。それらが `EditState` を呼ぶ。
- ADR-0068：platform trait パターン（Surface/FontFetcher）を `ImeBridge` が踏襲。
- ADR-0014/0016：IME は Platform Adapter の責務 — **精緻化**：IME *plumbing* は adapter（ImeBridge）、編集 *model* は core。
- ADR-0017：preedit は Element Layer — 完全実現（adapter の preedit 操作を撤去）。
- ADR-0032：cursor 点滅は render-side に残す。
- ADR-0063：text-input は leaf IFC。
- ADR-0012：native 本体。native は薄い ImeBridge 実装で済む。
