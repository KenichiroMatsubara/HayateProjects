---
status: accepted
---

# text-input の raw key 解釈を Platform Adapter に限定する

ADR-0103 の Decision 4 を権威ある境界とし、Platform Adapter だけが OS/raw key を `EditIntent` に変換する。Core の text-input 経路は `EditIntent` を適用するだけで、key string や modifier から編集操作を再構成しない。ADR-0103 Decision 2 の「`interaction.rs` はキー → EditIntent 写像に徹する」は Decision 4 と矛盾するため、「`interaction.rs` は受け取った EditIntent を dispatch する」と読み替える。

今回の実装適合範囲は text-input 編集に限定する。raw `KeyDown` は EditIntent に変換されなかったキーをアプリへ通知する経路として残す。SelectionArea の keyboard selection は edit ではないため `EditIntent` に混ぜず、そこに残る raw key 解釈の整理は別の `InteractionIntent` 作業とする。

multiline text-input の Enter は `EditIntent::InsertLineBreak` として表す。Platform Adapter は Enter をこの intent に写像し、Core は対象が multiline かつ IME composition 中でない場合だけ、現在の選択範囲を改行で置換して `TextInput` を通知し、intent を消費する。単一行または composition 中は未消費を返し、Adapter が raw `KeyDown` へフォールバックする。改行は typed `multiline` property によって可否が決まる編集コマンドなので、汎用 `InsertText("\\n")` には畳まない。

`EditIntent` は現時点で Tsubame renderer に利用箇所がなくても、将来の external semantic producer が利用できる正式な Hayate protocol capability として proto/wire に追加する。予約 opcode だけを置かず、proto/spec の closed encoding、Canvas/worker/native projection、Core の同じ `apply_edit_intent` seam への dispatch、未知値・対象不一致を含む wire conformance test まで実装する。Web DOM keymap は引き続き Rust Web Adapter が所有し、Tsubame/JS 側に重複 keymap を作らない。

wire endpoint は Element Layer mutation opcode ではなく、semantic input command `dispatch_edit_intent(target, intent) -> EditDispatchOutcome` とする。outcome は `Consumed`（Core が同期適用）、`Unhandled`（対象不一致、単一行の line break、IME composition 中など）、`Deferred`（Web Paste 等で Platform Adapter が非同期処理を開始）の closed vocabulary とする。raw key producer は `Unhandled` の場合だけ `KeyDown` へフォールバックできる。未知 intent tag または不正 payload は `Unhandled` ではなく protocol error とする。clipboard 実処理は各 Platform Adapter の既存境界に残し、Core の同期 seam は consumed/unhandled だけを返す。

## Considered Options

- Core と Platform Adapter の両方で同じ keymap を持つ: platform 差の正本が二つになり、ADR-0103 Decision 4 に反するため不採用。
- SelectionArea の keyboard 操作も `EditIntent` に含める: edit 専用 seam を広げ、CONTEXT の `InteractionIntent` / `EditIntent` 分離に反するため不採用。
- 改行を `InsertText("\\n")` として扱う: multiline policy と編集コマンドの意味を文字列へ隠すため不採用。
- 利用箇所が生じるまで wire capability を追加しない: protocol を先に安定させる方針を採り、完全実装された公開 capability として追加するため不採用。

## Consequences

- Core の text-input 用 `key_edit_intent` と raw-key fallback による編集処理を除去する。
- Core の raw Enter 特例と `EditState::apply_key_down` を除去し、改行は `InsertLineBreak` の適用だけで行う。
- Platform Adapter の keymap test が raw key → EditIntent を、Core test が EditIntent → EditState をそれぞれ検証する。
- proto/spec に EditIntent の closed encoding と全 projection を追加し、未使用でも conformance test で公開契約を維持する。
- semantic input command は declarative mutation packet と分離し、async platform operation を `Deferred` として観測可能にする。
- ADR-0103 の Chromium 編集意味論、multiline property、EditState の closed vocabulary は維持する。
