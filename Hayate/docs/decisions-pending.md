# Decisions Pending

2026-06-04 時点の未決定事項だけを残す。完了済み事項と廃止済み事項は ADR または archive に委ねる。

## Closed

- `protocol.yaml` を Hayate-Tsubame 間プロトコル定数の単一正本にする。
  根拠: ADR-0049（形式は ADR-0053 により JSON spec へ移行）
- Element Document Runtime を hayate-core に置き、poll deliveries で host に通知する。
  根拠: ADR-0053
- Tsubame は signal ランタイムではなく renderer target 基盤とする。
  根拠: ADR-0040
- Hayabusa は Hayate に Rust crate 依存で接続し、WIT 境界を使わない。
  根拠: ADR-0045
- `Scene Renderer` / `Render Host` / `Renderer Selection Policy` の語彙を採用する。
  根拠: ADR-0050
- WIT / `wit-bindgen` を Hayate-Tsubame 間の現行正本として扱う方針は廃止する。
  根拠: ADR-0049
- `event_kinds` の完全 codegen（`encode_event` spec 駆動、`wireRole` / `adapterTier` 反映、Rust `Event` フィールド名と `params[].name` 一致）。
  根拠: ADR-0053
- `modifier_keys` / `unset_kinds` の JSON spec 表現（bitmask 説明・`description` 必須 schema）。
  根拠: ADR-0047 / ADR-0053

## Open

### 1. アプリ固有フォント ID と `font_family` enum の接続

- `protocol.yaml` のプリセット `font_family` と、`hayate.config.json` 由来の app font ID をどう接続するか決める。
- 必要なら `100+` を app font 用予約帯にするなどの運用を ADR 化する。

根拠:
- ADR-0043
- ADR-0044
- ADR-0049

## Out Of Scope For This File

- WIT 削除 TODO の履歴
- `opcodes.ts` / `style-encoder.ts` 置換の実装手順
- 古い spec 文書の移行メモ

それらは各 ADR、issue、または archive 文書に残す。
