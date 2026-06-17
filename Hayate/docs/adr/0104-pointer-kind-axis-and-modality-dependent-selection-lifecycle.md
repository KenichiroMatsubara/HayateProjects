---
status: accepted
---

# ポインタ種別（PointerKind = Mouse/Touch/Pen）軸を core に導入し、選択 chrome と blur ライフサイクルを modality 依存にする

**Date: 2026-06-17**

> 本 ADR は決定のみを記録する（実装は後続）。現状 `InputModality` は
> `Pointer` / `Keyboard` の二値のみで（`:focus-visible` 用、#335）、**touch と mouse を
> 区別しない**。`PointerEvent.pointerType`（"mouse" / "touch" / "pen"）はアダプタ境界
> には届くが（`pointer_input_browser.rs` が `pointerType` を設定）、**core へ転送されず
> 捨てられている**。touch の唯一の手掛かりは `on_long_press` という別エントリのみ。
> その結果、**text-input の edit 選択が blur / 別所タップで collapse されず**、
> ハイライト描画が `el.edit.selection_range()` のみを見て **focus 非依存**で残り続ける
> バグ（`scene_build.rs`）がある。

## Context

Canvas Mode は GPU 描画でブラウザネイティブ選択が一切効かないため、選択モデル・
chrome・ライフサイクルを core が所有する（ADR-0097）。ところが「**フィールド外を
タップ / クリックしたとき選択がどうなるか**」は、**デスクトップ（mouse）とモバイル
（touch）で挙動が異なる**:

- **デスクトップ Chrome の `<input>` / `<textarea>`**: blur すると選択ハイライトは
  **非表示**になるが範囲は**記憶**され、再フォーカスで復活する（フォーム部品は
  「灰色保持」ではなく「非表示＋記憶」）。
- **モバイル / Android Chrome**: 選択は handle ＋ floating toolbar で示され、**外側を
  タップすると chrome を dismiss し caret に collapse** する。

Flutter も「テキスト編集スタックは mouse(desktop) と touch(mobile) を別扱いすべき」と
明示的にプラットフォーム分岐している。ADR-0097 は「chrome=Android-native お手本 /
highlight tint=Chromium お手本」を決めたが、**この touch-vs-mouse 軸そのもの**と、
**blur 時の選択ライフサイクル**は規定していなかった。Hayate にその軸が無いため、
「タップでグレー保持か、完全に解除か」がデスクトップ/モバイルで一緒くたになり、
体験がずれていた。

## Decision

1. **`PointerKind { Mouse, Touch, Pen }` を pointer intake に付与する。** `on_pointer_down`
   / `on_pointer_move` / `on_pointer_up` が PointerKind を受け、Platform Adapter が
   `PointerEvent.pointerType`（native は OS の pointer device）を転送する。core は
   `last_pointer_kind` を保持し、**Chrome と同じくインタラクション単位**で判定する
   （タッチ PC・マウス付きタブレット等の hybrid 端末に正しく追従。起動時固定にしない）。
   **pointer の proto/wire イベントに PointerKind を追加**する。`InputModality`
   （`:focus-visible` 用 Pointer/Keyboard）とは並立する別軸。

2. **選択 chrome（drag handle / floating toolbar）は Touch modality のときのみ描画する。**
   Mouse / Pen ではドラッグ選択＋細いキャレットのみ。ADR-0097 の「handle はモバイル
   ジェスチャ面」を modality ゲートとして具体化する。

3. **blur（フォーカス喪失 / フィールド外操作）時の選択ライフサイクルを modality 依存に
   する。** **Mouse / Pen**: 選択ハイライトを非表示にし、`EditState` の範囲は**記憶**、
   再フォーカスで復活（Chromium フォーム部品パリティ）。**Touch**: caret に
   **collapse** し chrome を dismiss（Android お手本 — ユーザの実体験）。

4. **選択ハイライト描画を focus 連動にする。** focused の text-input のみ active
   ハイライトを描く。これにより Mouse-blur の「非表示＋記憶」と Touch-blur の
   「collapse」の双方が成立し、現状の「unfocused でハイライトが残る」バグが解消する。
   single-active（ADR-0097）は「**active（= focused）な選択は文書全体で一つ**」に帰着し、
   Mouse 経路で別フィールドへ移っても旧フィールドの範囲は inactive（非表示・記憶）として
   矛盾しない。

## Considered Options

- **起動時に form-factor（mobile / desktop）を 1 回固定する** — 実装は軽いが、タッチ PC・
  マウス付きタブレットで誤る。Chrome 自身がインタラクション単位で pointer type を見て
  いるのと不一致。却下。
- **ジェスチャ（`on_long_press`）から touch を推定する** — 新 API 不要だが、マウス長押しや
  touch のダブルタップ選択を取り違え、chrome を一般にゲートできない。却下。
- **blur で常に collapse（modality 無視）** — モバイル要望は満たすが、デスクトップ
  Chromium の「非表示＋記憶＋再フォーカス復活」から乖離する。却下。
- **blur で常に灰色保持（当初案）** — デスクトップのフォーム部品ですら不正確（Chrome は
  灰色保持ではなく非表示＋記憶）で、モバイルの「外側タップで消える」体験にも反する。
  この案の破綻が本 ADR の出発点。却下。

## Consequences

- `proto/spec` の pointer イベントに `PointerKind` を追加する。
- core に `last_pointer_kind` と、blur 時の modality 分岐（Mouse=非表示＋記憶 /
  Touch=collapse）が入る。
- `selection_chrome` の表示が Touch ゲートになる。highlight 描画が focus 連動になる。
- ADR-0097 を**補強**する（chrome=Android お手本に「modality ゲート」と「blur
  ライフサイクル」を追加）。tint=Chromium お手本（ADR-0097）は不変。
- カーソル形状（ADR-0105）は Mouse / Pen modality でのみ意味を持つ、という前提を与える。
- 実装は本決定後（本 ADR は決定のみ）。

## 関係

- ADR-0097（統一テキスト選択）: 本 ADR が PointerKind 軸・chrome ゲート・blur
  ライフサイクルを追加して補強。selection モデル本体・tint お手本は不変。
- ADR-0088（pointer から cursor 解決）: 同じ pointer intake を PointerKind で拡張する。
- ADR-0102（Canvas 視覚お手本=DOM）: tint=Chromium と整合。
- ADR-0103（EditIntent / 編集キー Chromium 準拠）: 「編集キー軸」と本 ADR の
  「選択ジェスチャ / modality 軸」は直交。
- #335（InputModality / `:focus-visible`）: Pointer / Keyboard の二値と並立して
  PointerKind を持つ。
