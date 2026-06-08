# Decisions Pending

2026-06-07 時点の未決定事項だけを残す。完了済み事項と廃止済み事項は ADR または archive に委ねる。

## Closed

- Hayate-Tsubame 間プロトコル定数の機械可読な単一正本を導入する。
  根拠: ADR-0049（形式は ADR-0053 により `proto/spec/*.json` + `@hayate/protocol-spec` へ移行）
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
- Scene Renderer 契約の粒度: `SceneGraph` 入力のまま、walk は `hayate-core` の `ScenePainter` seam に集約。実装は `scene-renderers/{vello,tiny-skia}`。
  根拠: ADR-0054
- 単一正本の scope を wire codec（mutation/style encode + decode）まで拡張。`style_tags.encodeFrom` で TS 入力変換を spec 化。
  根拠: ADR-0055

## Open

### 1. アプリ固有フォント ID と `font_family` enum の接続

- spec のプリセット `font_family` と、`hayate.config.json` 由来の app font ID をどう接続するか決める。
- 必要なら `100+` を app font 用予約帯にするなどの運用を ADR 化する。

根拠:
- ADR-0043
- ADR-0044
- ADR-0049

### 2. Render Host の web surface を scene-renderers に移管するか 〔解決 → ADR-0068〕

- ~~現状は ADR-0054 H1: web surface 初期化・present は `hayate-adapter-web` に残留。~~
- **決定（ADR-0068）**: native が確定設計目標である以上、プラットフォーム非依存の Render Host 芯（policy/orchestration/資源寿命）と Font ロードを**共有層へ今 hoist**し、web 特有部分（surface 生成・fetch）を `Surface` / `FontFetcher` trait の裏に置く。surface **生成**の web 実装は `impl Surface` として adapter-web に残る（H1 の事実は trait 化して維持）。2-adapter 則の例外＝variation が roadmap 上確定しているため投機ではない。

根拠:
- ADR-0068（ADR-0054 H1 を revisit）
- ADR-0050 / ADR-0054 / ADR-0066

## Out Of Scope For This File

- WIT 削除 TODO の履歴
- 古い spec 文書の移行メモ
- ADR-0054 / ADR-0055 の実装手順（各 ADR の Implementation Tasks を参照）
