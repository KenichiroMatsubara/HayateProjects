---
status: accepted
---

# text-input 編集を EditIntent 閉じた語彙に集約し、キー意味論は Chromium 準拠・OS キーマップは Platform Adapter 所有・単一行/複数行は multiline property で表す

**Date: 2026-06-17**

> **追補（ADR-0151）:** Decision 2 の「`interaction.rs` はキー → EditIntent 写像に徹する」は Decision 4 と矛盾するため、「`interaction.rs` は Platform Adapter から受け取った EditIntent を dispatch する」と訂正する。Core の text-input 経路は raw key を編集操作へ写像しない。

> 本 ADR は決定のみを記録する（実装は後続）。`EditState`（ADR-0069）には
> `insert` / `backspace` / `delete_selection` / `cut` / `paste` / `move_focus` /
> `set_selection` 等の変異プリミティブが既にあり、`selection.rs` に
> `next_grapheme` / `prev_grapheme` / `next_word` / `prev_word` / `arrow_step` が
> ある。一方で **素の矢印キーでキャレットが動かない**（`arrow_step` 呼び出しが
> いずれも `MOD_SHIFT` 必須）・**Delete（前方削除）が無い**（`apply_key_down` は
> `Backspace` / `Enter` のみ）・**text-input にフォーカス中の Ctrl/Cmd+A/C/X/V が
> 不通**（`handle_selection_key` が `self.selection` 前提で、text-input は
> `EditState` 側で選択を持つため素通り）・**Enter が末尾 append**（キャレット位置に
> 挿入しない）という穴がある。

## Context

text-input を「選択しかできない」状態から実用的な編集フィールドへ引き上げるには、
caret 移動・前方削除・行/単語/文書境界の移動・クリップボードのキー経路など多数の
操作が一気に必要になる。これらをどの層に、どの形で載せるかを決めないと、
`EditState` の API 面が膨張し、`interaction.rs` に OS 分岐ロジックが散る。

Hayate には軸が複数ある。**Canvas の視覚お手本は Chromium DOM**（ADR-0102）、
**選択 chrome（handle / toolbar / 拡大鏡）のお手本は Android-native**（ADR-0097）、
**SelectionArea モデルは Flutter**。このうち「**キーレベルの編集挙動**（矢印・Delete・
Home/End・単語境界・Ctrl ショートカット）の正準」だけが未決だった。

加えて CONTEXT「Platform Adapter は raw 入力変換を担い、Core は Platform Adapter を
知らない」と、ADR-0069「編集セマンティクスは core」の間に、OS 依存キーバインド
（macOS は Cmd+←=行頭・Option+←=単語、Windows/Linux は Home=行頭・Ctrl+←=単語で
**キー自体が異なる**）をどちらが解釈するかという緊張がある。`MOD_PRIMARY`（Cmd/Ctrl の
抽象）だけではこの差は吸収しきれない。

現状 text-input は**単一行/複数行の区別を持たず**、`Enter` が常に末尾へ改行を
append する。これが `<input>`（Enter は改行しない）と `<textarea>`（Enter は
キャレット位置へ改行）の挙動分岐を不能にしている。

## Decision

1. **キーレベル編集挙動の正準は Chromium `<input>` / `<textarea>`。** OS 差（Mac の
   Cmd/Option vs Win/Linux の Ctrl/Home/End）は Chromium が当該 OS で示す挙動を
   そのまま正準とする。これは「**編集キー意味論**」の軸であり、「**選択ジェスチャ /
   chrome ライフサイクル**」の軸（ADR-0097 = Android-native お手本、ADR-0104）とは
   **直交する別軸**として扱う。

2. **編集操作を EditIntent 閉じた語彙にし、`EditState::apply(EditIntent)` を唯一の
   編集シームにする。** Move / Extend × `{Grapheme, Word, LineBoundary, DocBoundary}`
   × `{Backward, Forward, Up, Down}`、Delete × `{Backward, Forward}` × `{Char, Word}`、
   `SelectAll` / `Copy` / `Cut` / `Paste`。ADR-0071 の closed vocabulary の一員。
   `interaction.rs` はキー → EditIntent 写像に徹し、EditState は個別メソッドを
   増殖させない。`selection.rs` の grapheme / word ステップ群を EditIntent 実装が
   再利用する。

3. **単一行/複数行は `multiline` typed property で表す（既定 false）。** ADR-0096 /
   0097 の「専用 element-kind を増やさず closed typed property」哲学（ADR-0071）と
   同型。`false`（= `<input>`）: Enter は改行せず submit 意図を `KeyDown` で通知、
   ↑=先頭 / ↓=末尾、Home/End=フィールド全体端。`true`（= `<textarea>`）: Enter は
   **キャレット位置へ**改行（現状の末尾 append バグを是正）、↑/↓ は表示行移動、
   Home/End は表示行端。

4. **OS キーバインド → EditIntent の写像は Platform Adapter が所有し、core は OS
   非依存で EditIntent を適用する。** core は *what*（intent の意味）、adapter は
   *which*（どのキーがどの intent か = OS keymap）。CONTEXT「raw 入力変換 = Adapter /
   Core は Adapter を知らない」と整合する。**EditIntent は Hayate–Tsubame proto/wire
   契約に加わる**（Canvas 経路）。raw `on_key_down` は非編集キーとアプリ向け
   `KeyDown` 通知に残し、**編集解釈は EditIntent へ移す**。DOM / HTML 経路は
   ブラウザネイティブ編集に委ね、意味論のみパリティ（ADR-0097 と同型）。

5. **目標挙動のスコープ。** 以下を本決定の対象に含める: ①単語境界の move / delete
   （Ctrl/Option+矢印・Ctrl/Alt+Backspace/Delete）、②複数行の ↑/↓（sticky
   goal-column =「desired x」を保つ縦移動）＋ Home/End、③クリップボードのキー経路
   統一（Ctrl/Cmd+C/X/V を EditIntent 経由で text-input にも効かせ、Canvas は非同期
   clipboard read を結線して Ctrl+V を機能させる）、④text-input 内のダブル/トリプル
   クリック単語・行選択（Mouse modality）。**undo / redo は non-goal**（編集履歴
   管理は別単位の成長点）。

## Considered Options

- **個別メソッドを増やす（`move_left` / `delete_forward` / `delete_word_back` …）** —
  小さく始められるが、操作が増えるほど `EditState` の API 面が膨張し、`interaction`
  側に collapse 判定・境界分岐が散る。却下。
- **core が per-OS キーマップ表を持ち、起動時に platform ヒントを受ける** — 意味論と
  キーマップが core に単一ソース化されパリティは確実だが、Core が OS を知ることになり
  CONTEXT「Core は Platform Adapter を知らない」から逸脱する。却下。
- **`MOD_PRIMARY` 抽象のみ（現状延長）** — Mac「Cmd+←=行頭」と Win「Ctrl+←=単語」が
  同じ `primary+←` に潰れて区別不能になり、Chromium パリティが崩れる。却下。
- **常に複数行（textarea 意味論固定）／ `text-input` と `text-area` の 2 element-kind** —
  前者は実アプリで多い「1 行入力欄」を表せず、後者は専用 kind 哲学（ADR-0096/0097）に
  逆行する。却下。

## Consequences

- `proto/spec` に EditIntent 閉じた語彙と `multiline` element property を追加する。
  Canvas wire に EditIntent op が増える。
- `EditState::apply(EditIntent)` が唯一の編集シームとなり、意図単位で単体テストできる。
- adapter に OS keymap（macOS / Windows / Linux）が入る。DOM / HTML 経路は持たない。
- raw `on_key_down` の編集解釈（`Backspace` / `Enter` 直書き）が EditIntent へ移り、
  `Enter` のキャレット位置挿入・前方 Delete・素の矢印移動・Home/End が揃う。
- undo / redo は編集履歴として別 ADR / 別作業単位。
- 実装は本決定後（本 ADR は決定のみ）。

## 関係

- ADR-0069（EditState 集約）: 編集の変異プリミティブを `apply(EditIntent)` シームへ
  発展させる。ImeBridge 境界は不変。
- ADR-0097（統一テキスト選択）: 「編集キー意味論（Chromium）」と「選択ジェスチャ /
  chrome（Android）」の軸分離を本 ADR が明文化。
- ADR-0104（ポインタ種別と選択ライフサイクル）: blur / chrome は modality 軸で、
  本 ADR のキー軸と直交。
- ADR-0102（Canvas 視覚お手本=DOM）: 編集キーも Chromium 準拠とすることで Canvas /
  DOM のパリティが取れる。
- ADR-0071（closed vocabulary）: EditIntent・`multiline` はその一員。
- ADR-0096（pointer / drag と typed property 哲学）: `multiline` を新 kind ではなく
  property で表す根拠。
