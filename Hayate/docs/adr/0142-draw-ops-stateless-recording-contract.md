# draw の記録 API はステートレス Flutter/Skia 流とし、op 表を proto spec 正本から両側 codegen する

**Status: accepted**

**Date: 2026-07-06**

## Context

draw（ADR-0141）の記録 API には 2 大流派がある。Flutter/Skia 流（`Path` + `Paint` が第一級オブジェクトで、`drawPath(path, paint)` のように呼び出しごとに paint を明示するステートレス設計）と、HTML Canvas 2D 流（`ctx.fillStyle = ...; ctx.fill()` のようにコンテキストが塗り状態を保持するステートフル設計）。

Hayate の消費経路は Tsubame（TS）に加え、将来 Hayabusa の Script Adapter（WASM にコンパイル可能な任意の単一言語）へ広がる。Script Adapter の規律は「言語ごとに意味論を再実装しない」——リアクティビティと同様、描画記録も各言語の表面は薄い projection でなければならない。

## Decision

**記録 API はステートレス Flutter/Skia 流とする。** canvas 自体が持つ状態は save/restore の変換・クリップスタックのみ。

- 描画 opcode 表（パス動詞・描画命令・引数の名前と型）と Paint のフィールド表・enum（cap / join / fill rule 等）を proto spec の新 JSON として正本化し、既存 `style_tags` と同様に Rust decode / TS encode を両側 codegen する。display list は f32 flat buffer。
- 各言語の recorder（TS の `Canvas` / `Path` / `Paint` など）は **spec の op 表から表駆動で生成される薄い encoder** とし、手書きの意味論を持たない。
- 命名住み分け: アプリ向け TS 表面は Flutter 語彙（`Canvas` / `Path` / `Paint`）をそのまま使い、Hayate 内部・ドキュメントは draw list 系語彙を使う（既存概念「Canvas Mode」との用語衝突回避）。

## Rationale

- **多言語対応が決定打。** ステートレス API は「op 名 + 型付き引数リスト」の集合にすぎず、spec 表から各言語のメソッドを機械生成できる。encoder / decoder とも表駆動の薄い変換で済む。
- ステートフル 2D 流は「その時点の fillStyle は何か」「save/restore がスタイル状態も巻き戻す」という**表に載らない暗黙状態機械**を encoder / decoder 両側・全言語に手書きで強いる。しかも Rust にプロパティ代入はなく `set_fill_style(...)` になった時点で 2D 流の馴染みという利点も消える。
- ステートレスな `Path` は immutable な記録済み op 列としてフレーム間・要素間で再利用でき、`shouldRepaint` と組んだ再記録抑制が効く。
- 将来階層の拡張点が Paint に自然に集約される: グラデーションは brush フィールド、blend は blendMode フィールド、フィルタは maskFilter——Flutter と同じ拡張面で、op 表への追加 = 契約破壊なしの語彙拡張になる。

## Considered Options

- **HTML Canvas 2D 流（ステートフル）**: 上記の通り暗黙状態機械が多言語 codegen を破壊する。却下。
- **op 表を契約外の不透明バイナリにする**: 「機械可読 spec 正本 + 両側生成 + 閉じた語彙」の規律の対極で、encode/decode drift を検出できない。却下。
