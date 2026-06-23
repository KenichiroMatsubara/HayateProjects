# §10 Protocol & Wire Contract

Hayate（Rust + WASM）と Tsubame（TypeScript）の**唯一の結合点**。Hayate が契約を所有し、Tsubame が消費する非対称な関係である。HayateProjects モノレポは AI/クラウド作業都合であり、アーキテクチャ上の結合点ではない（結合点はこの契約だけ）。

正本: `Hayate/proto/spec/*.json`。データフローは `apply_mutations`（host → Hayate、毎フレーム1回）と `poll_events`（Hayate → host、drain）の二方向。

凡例: ✅実装済み / 🟡部分 / ⬜未実装。状況の定義は [index](./index.md#読み方) 参照。

---

## 単一正本と生成

### PROTO-01 — JSON spec 単一正本
**規範文:** Hayate⇄Tsubame 契約の正本は `Hayate/proto/spec/` の JSON 8 セクション（`opcodes` / `style_tags` / `event_kinds` / `element_kinds` / `unset_kinds` / `modifier_keys` / `types` / `enums`）である。Tsubame は正本を持たず、npm パッケージ `@hayate/protocol-spec` を workspace 依存として取り込む。
**出典:** ADR-0049, ADR-0053
**状況:** ✅ — `Hayate/proto/spec/*.json`（8ファイル + `schema/`）。Tsubame 生成器は `@hayate/protocol-spec/load` を import（`Tsubame/proto/generator/gen-*.mjs`）。
**備考:** —

### PROTO-02 — WIT は契約正本ではない
**規範文:** WIT / wit-bindgen は Hayate⇄Tsubame の現行契約正本ではない。`Hayate/wit/` は存在しない。WIT に言及する場合は歴史的設計として扱う。
**出典:** ADR-0049（supersedes ADR-0013, 0015, 0033, 0039 の WIT 部分）
**状況:** ✅ — `Hayate/wit/` ディレクトリ不在、リポジトリ内に `*.wit` なし。
**備考:** [履歴 C-10.1] WIT を正本とする旧方針は 0049 で廃止。wit-bindgen 再導入は将来検討事項として開いたまま（契約正本としてではない）。

### PROTO-03 — 二世代生成 + diff ゲート
**規範文:** 正本 spec から Rust 生成器（`Hayate/proto/generator/` → `Hayate/proto/generated/`）と TS 生成器（`Tsubame/proto/generator/` → `Tsubame/proto/generated/`）が両言語コードを生成する。両 `generated/` は commit し、CI が `check:proto`（validate → generate → diff）で陳腐化を検出する。
**出典:** ADR-0049, ADR-0053
**状況:** ✅ — 両 `generated/` 存在（Rust: `codec.rs`/`protocol.rs`/`dispatch.rs`/`dom_style_mapper.rs`/`event_types.rs`、TS: `codec.ts`/`protocol.ts`/`catalog.ts`/`delivery.ts`）。`package.json` に `check:proto` スクリプト。
**備考:** generator が2箇所に分かれるのは 0049 が受容したトレードオフ（spec は単一、生成器は言語ごと）。

---

## apply_mutations（host → Hayate）

### PROTO-04 — apply_mutations 署名
**規範文:** `apply_mutations(ops: Float64Array, styles: Float32Array, texts: string[])` の3引数。hot path（1回/frame）のため typed array で転送効率を最優先する。
**出典:** ADR-0052（supersedes ADR-0039 の2引数署名）
**状況:** ✅ — Rust `element_renderer.rs:504`、TS `hayate-mutation-packet.ts:181`。
**備考:** [履歴] 0039 は `(ops, styles)` の2引数だった。string table 導入（PROTO-07）で `texts` を追加。

### PROTO-05 — ops 固定長レコード
**規範文:** `ops` は `op_kind` 始まりの固定長レコード列。各 op 種別の slot 数は `OP_SLOTS` テーブルが駆動する。不明な `op_kind` はそのフレームの残りを捨てる（固定長前提のためズレを波及させない）。
**出典:** ADR-0039, ADR-0049
**状況:** ✅ — `opcodes.json`（各 op に slots）、生成 `parse_next_op`。
**備考:** —

### PROTO-06 — OP_CREATE は batch 内
**規範文:** `element_create` は `OP_CREATE=9`（`op, id, kind_code`）として ops バッチに含める。ElementId は JS 側がモノトニックカウンターで採番し WASM へ通知する。
**出典:** ADR-0005（supersedes ADR-0039 の「element_create は batch 外」）
**状況:** ✅ — `opcodes.json` CREATE=9。
**備考:** [履歴 C-10.3] 0039 は戻り値が要るため batch 外個別呼び出しとしていた。JS 採番（ADR-0005）で batch 内へ。

### PROTO-07 — string table
**規範文:** 文字列 op は `texts[]` に集約し、`OP_SET_TEXT=10`（`op, id, text_index`）と `OP_UNSET_STYLE=11`（`op, id, kind`）が index/kind で参照する。呼び出し順序管理は Rust 側に置き、TS は ops と texts を組み立てて一括送信するだけでよい。
**出典:** ADR-0052（supersedes ADR-0039 の「文字列 op は apply_mutations 外」）
**状況:** ✅ — `opcodes.json` SET_TEXT=10 / UNSET_STYLE=11、TS `hayate-mutation-packet.ts:160-161`（texts.push）。
**備考:** [履歴 C-10.4] 0039 は文字列 op を個別 WASM 呼び出しにしていたため、TS 側に「typed batch を先に、string を後に」という順序ポリシーが漏れていた。0052 がそれを Rust に移譲。

### PROTO-08 — style packet
**規範文:** `apply_mutations` 第2引数は flat f32 の style packet（`style_packet.rs` の TAG エンコード）。`OP_SET_STYLE` の `style_offset` / `style_len` で部分参照する。
**出典:** ADR-0039, ADR-0049
**状況:** ✅ — `style_tags.json`、生成 `parse_next_style_tag` / `decode_style_packet`。
**備考:** —

---

## Wire Codec（spec 駆動の符号化）

### PROTO-09 — wire codec 単一正本
**規範文:** encode/decode のアルゴリズムを spec から生成する（Rust: encode + decode、TS: encode）。手書き codec shim は持たない。`style_tags.encodeFrom` が TS 入力変換規則を spec 化する。
**出典:** ADR-0055
**状況:** 🟡 — 生成 codec は両側に存在（`Hayate/proto/generated/codec.rs`、`Tsubame/proto/generated/codec.ts`）。ただし手書き `renderer-canvas/src/hayate.ts` に `parseColor`/`parseDimension` が残存し、生成 codec と二重定義の疑い（ADR-0055 Task 4.3「dead code 削除を判断」が未完）。
**備考:** 精査結果（2026-06-09）— `hayate.ts` の `parseColor`/`parseDimension` は生成 `codec.ts` 版と byte 一致の重複で、リポ内のどこからも import されていない（package `index.ts` から export のみ）。生成 codec に吸収済みの dead code。削除が筋だが exported package API のため除去は外部影響確認のうえ別途（C5 フォローアップ）。

### PROTO-10 — codec 検証（apply_mutations 方向, C1/C2）
**規範文:** `proto/spec/fixtures/{ops,style}_encode.json` を期待 wire の正本とし、Rust が roundtrip（C1: wire→decode→encode）、TS が encode 出力照合（C2）で両側を fixture に固定する。
**出典:** ADR-0055（C1/C2/C4）
**状況:** ✅ — C1 `Hayate/crates/platform/web/src/wire_codec_roundtrip.rs`、C2 `Tsubame/packages/renderer-canvas/src/codec-fixtures.test.ts`（同一 `style_encode.json` を参照）、C4 fixtures commit 済み。
**備考:** apply_mutations 方向は両言語が同一 fixture に固定されており drift しない。

### PROTO-11 — codec 検証（C3 結合）
**規範文:** TS flush → WASM `apply_mutations` の結合テスト（C3）で、生成 codec 経由の実 wire が WASM に通ることを保証する。
**出典:** ADR-0055（C3）
**状況:** ✅ — `Tsubame/packages/renderer-canvas/src/wasm-integration.test.ts` が `HayateMutationPacket` flush → 実 `apply_mutations`（`pkg-null` / `backend-null`）を検証。`element_get_text` / `element_subtree_ids` でツリー反映を確認。CI: `.github/workflows/wasm-c3.yml`（wasm 変更時ゲート）。
**備考:** delivery poll の単体は `canvas-renderer.test.ts` が `StubHayate` を継続利用（関心の分離）。C3 は wasm プロジェクトに分離。

---

## poll_events（Hayate → host）

### PROTO-12 — delivery poll（export-only）
**規範文:** Hayate は host へ import callback しない。`register_listener(element_id, event_kind) -> ListenerId` を export し、runtime が bubble dispatch 後にキューへ Event Delivery `{ listener_id, event }` を積む。host は `poll_events()` で drain し `ListenerId` に紐づく handler を実行する。
**出典:** ADR-0018, ADR-0053
**状況:** ✅ — `register_listener` export、delivery encode は生成 `protocol.rs:972`（`encode_events`）/ `:980`（`encode_deliveries`）。
**備考:** [履歴] 0018 の raw event poll から 0053 で delivery poll へ進化（export-only 原則は維持）。

### PROTO-13 — poll_events の形状
**規範文:** `poll_events()` は配列の配列を返す。各行は `[listener_id, kind, ...fields]`。
**出典:** ADR-0034
**状況:** ✅ — TS `parseDelivery(row)` が `row[0]=listener_id, row[1]=kind, row[2..]=fields` を前提に decode（生成 `delivery.ts`）。
**備考:** —

### PROTO-14 — event encoder は spec 駆動
**規範文:** `encode_event` / `encode_events` を spec から生成する。`event_kinds` は `wireRole` / `adapterTier` / `interactionKind` を持つ。
**出典:** ADR-0049, ADR-0053
**状況:** ✅ — `Hayate/proto/generated/protocol.rs:720`（`encode_event`）、`event_kinds.json` に3メタフィールド完備。
**備考:** —

### PROTO-15 — Event フィールド名一致
**規範文:** `hayate-core` の `Event` enum フィールド名は spec の `params[].name` に一致させる（例 `Event::Click { target_id, x, y }`）。乖離はコンパイルエラーで検出される。
**出典:** ADR-0049
**状況:** ✅ — `document_runtime.rs:124`（`Event::Click { target_id, .. }`）。
**備考:** —

### PROTO-16 — wireRole による到達制御
**規範文:** event は `wireRole` で host への届け方を決める。`interaction` / `ime` は delivery、`hayate-internal`（例 `fetch_font`）は届けず runtime/adapter 内で完結、`host-echo`（例 `resize`）は届けず `IRenderer.resize` 指令を正とする。
**出典:** ADR-0053
**状況:** ✅ — `event_kinds.json`: `fetch_font` = `hayate-internal`、`resize` = `host-echo`、`composition_*` = `ime`。
**備考:** —

### PROTO-17 — codec 検証（event/delivery 方向）
**規範文:** delivery wire を `proto/spec/fixtures/delivery_encode.json`（`[{name, kind, fields, wire}]`、positional、全 event kind）の共有 fixture で固定し、Rust は `event → encode_event → wire` 照合、TS は `wire → parseEvent → kind+fields` 照合（ADR-0055 検証層 C5）。両側が同一 fixture を本番方向で参照し、両言語の delivery wire 一致を保証する。
**出典:** ADR-0055（2026-06-07 amend で検証層 C5 を追加）
**状況:** ✅ — `proto/spec/fixtures/delivery_encode.json`（全16 event kind）；Rust `delivery_codec_fixtures.rs`（`encode_event_wire` 照合）；TS `delivery-fixtures.test.ts`（`parseEvent` 照合）；`delivery.test.ts` は fixture 駆動。
**備考:** [解決 C-10.5] generator に `encode_event_wire`（native テスト用）を追加し `encode_event` はそれを委譲。

---

## 境界と未決

### PROTO-18 — semantic 層は Contract 外
**規範文:** `StylePatch` / `HayateMutationPacket` / `IRenderer` の tree・style・imperative メソッド型、`setProperty`・`addEventListener` 購読 API・`resize` は spec 化しない（Renderer Protocol の領分、§11）。
**出典:** ADR-0053, ADR-0055
**状況:** ✅ — spec は wire（定数・codec・delivery）のみを所有し、semantic 型を含まない（意図通りの境界）。
**備考:** これは「やらないこと」の規範。spec の肥大化と Renderer Protocol への侵食を防ぐ。

### PROTO-19 — app font と font_family の接続
**規範文:** `font_family` は wire 上も登録上も**文字列**を正とする。app font は `hayate.config`（ADR-0044）で `{family, url}` を宣言し `register_font` に**名前**で接続する（数値 app font ID は導入しない）。web プリセットの名前は spec `enums.json` `font_family` が正本、名前→CDN URL は web adapter の `fonts.json` から `builtin_font_url` を codegen し、`fonts.json` は spec プリセット名を網羅する。
**出典:** ADR-0061（decisions-pending Open #1 を解決。ADR-0043/0044/0049）
**状況:** ✅ — `fonts.json` + `build.rs` codegen → `builtin_fonts.rs`；`enums.json` 全 preset の URL カバレッジテスト。文字列接続（`register_font`/`configure_fonts`、app登録 > builtin fetch > bundled default）も実装済み。
**備考:** [解決 C-10.6] 「100+ 予約帯」案は obsolete として正式 reject（ADR-0061）。URL は web adapter 層に留め spec に入れない（ADR-0043 維持）。

### PROTO-20 — tag→CSS DOM 写像は spec が所有し両側生成
**規範文:** Hayate CSS tag → ブラウザ CSS の DOM 写像（CSS プロパティ名・値フォーマット・`DOM_EXTRAS` 副作用）は `style_tags.json` の各 entry の `domCss`（`{ property, format, extras }`、DOM 写像なしは `null`）が単一正本。`dom_style_mapper.rs`（Rust・Hayate HTML Mode）と `catalog.ts`（TS・Tsubame DOM Renderer）を spec から生成し、Rust generator のハンドコード dispatch と `gen-catalog.mjs` の `DOM_EXTRAS` config を撤去する。
**出典:** ADR-0070（ADR-0049/0055 の単一正本を DOM 写像まで拡張）
**状況:** ✅ — `style_tags.json` 全 entry に `domCss`（写像なしは省略可）。`proto/generator/src/lib.rs` が `dom_style_mapper.rs`（Rust・Hayate HTML Mode）、`gen-catalog.mjs` が `catalog.ts`（TS・Tsubame DOM Renderer）を spec 駆動生成。Rust ハンドコード dispatch と `gen-catalog.mjs` の `DOM_EXTRAS` config は撤去済み。parity テスト `hayate-css-parity.test.ts`。
**備考:** DOM 写像は web 専用だが、単一化で Hayate HTML Mode と Tsubame DOM Renderer が一致し web の Canvas↔DOM デザイン比較（ADR-0012）が信用できる。

---

## このパートの集計

| 状況 | 件数 | ID |
|---|---|---|
| ✅実装済み | 19 | PROTO-01〜08, 10〜20 |
| 🟡部分 | 1 | PROTO-09 |
| ⬜未実装 | 0 | — |

**所見:** §10 の wire 契約は実装完了。残るギャップは PROTO-09（手書き `hayate.ts` parser の dead code 整理）のみ。
