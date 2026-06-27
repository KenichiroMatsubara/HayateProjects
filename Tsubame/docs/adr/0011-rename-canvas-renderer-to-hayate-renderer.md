# Canvas Renderer を Hayate Renderer に改名する（ターゲット名規約）

**Status: accepted**

**Date: 2026-06-27**

## Context

`Canvas Renderer`（`@tsubame/renderer-canvas`）は ADR-0002 で「Renderer Protocol + DOM Renderer + Canvas Renderer の 3 層構成」として命名された。だがこの名前は構造的な誤解を生む:

1. **HTML `<canvas>` との混同**。実体は GUI の `<canvas>` 要素とは無関係で、フレーム分の mutation を `apply_mutations(ops, styles)` に符号化して `RawHayate` ポートへ渡す **プラットフォーム盲目な encoder**。`HTMLCanvasElement` 型も canvas 参照も持たない（CONTEXT.md「host を知らない」）。CONTEXT.md は既にこの紛らわしさを自白し、surface を指す別語 "h-canvas / Hayate canvas" を用意していた。
2. **「Canvas Renderer = Android 対応」という誤読**。同一の renderer が web-WASM・Android native・将来 iOS/Desktop で**不変**に動く。プラットフォームを成立させるのは `raw` を供給する `Host`（web は `createHayateWebHost` の WASM ロード、Android は native cdylib + vsync pump）であって renderer ではない。"Canvas" という surface 由来の名前はこの軸（renderer は不変・Host が platform）を覆い隠す。
3. **対の非対称**。片割れ `DomRenderer` は**ターゲット**（DOM）で命名されている。"Canvas" だけが surface メタファで、規約が揃っていない。

`Canvas` 系の語は monorepo に 3 概念が併存し、字面一括置換は危険:

| 文字列 | 正体 | 改名 |
|---|---|---|
| **Canvas Mode**（69件） | `hayate-adapter-web` の動作モード（Vello+wgpu GPU 描画） | しない |
| **Canvas Renderer**（37件） | Tsubame の renderer 実装 | する → Hayate Renderer |
| h-canvas / Hayate canvas | Hayate が描く surface | しない（既に Hayate 印） |
| `<canvas>` | HTML 要素 | しない |

## Decision

1. **`Canvas Renderer` → `Hayate Renderer` に改名する。** ターゲット名規約に揃える（`DomRenderer`→DOM、`HayateRenderer`→Hayate）。クラス `CanvasRenderer` → `HayateRenderer`、パッケージ `@tsubame/renderer-canvas` → `@tsubame/renderer-hayate`。
2. **改名対象は語「Canvas Renderer」「`CanvasRenderer`」のみ。** `Canvas Mode`・`Canvas 経路`・`GPU Canvas`・`Canvas/HTML 経路`・h-canvas・HTML `<canvas>` は不変（別概念）。
3. **生きた語彙は完全改名、歴史記録は追記注記。**
   - **CONTEXT.md（root / Hayate / Tsubame）・コード・コメント・proto・パッケージ名**: 完全改名（語彙の正本がドリフトの震源なので必ず直す）。
   - **本 ADR**: 改名の決定と理由を記録する正本。
   - **既存 ADR の本文**: 書き換えない。supersede 規律と歴史的整合性を保つため、`Canvas Renderer` を語として使う ADR の初出に forward 注記を 1 行追記するに留める。
4. **ADR-0002 の決定は覆さない。** 3 層構成（Renderer Protocol + 二 renderer 実装）は不変で、変わるのは一方の実装の**名前だけ**。よって supersede ではなく用語改名。

## Considered Options

- **`HCanvasRenderer` / `Hanvas`**: CONTEXT の "h-canvas" 語彙には揃うが、"Canvas" を残すため HTML `<canvas>` 連想と「Canvas=Android?」誤読が残存し、`DomRenderer` との命名規約（ターゲット名）も揃わない。→ 却下。
- **改名せず docs で説明強化**: churn ゼロだが、漏れた名前が残り「なぜ canvas?」が再発する。ドリフト震源（CONTEXT.md）が "Canvas" のままだと毎セッション復活する。→ 却下。
- **全 ADR 本文を surgical 置換**: 語彙は完全一致するが、決定当時の記録を falsify し、69 件の `Canvas Mode` と隣接するため誤爆リスクが高い。リポの supersede（append-only）規律にも反する。→ 却下。forward 注記を採る。

## Consequences

- `DomRenderer` / `HayateRenderer` がターゲット名で対称になり、「`HayateRenderer` 採用＝自動 Android 対応」という誤読が構造的に言えなくなる（Hayate が走る全 platform を指すのが自明）。
- パッケージ rename（`@tsubame/renderer-canvas` → `@tsubame/renderer-hayate`）に伴い、import・`package.json`・`tsup`/`vite` 設定・型 re-export の機械的更新が必要。これは別 PR の実装スコープ。
- 既存 ADR は本文不変＋forward 注記。`_Avoid_: Canvas Renderer（旧称）` を CONTEXT.md の Hayate Renderer 項に明記済み。
- **monorepo `docs/adr/0004`（Tsubame–Hayate 境界・status: proposed）§5「命名」は「`Canvas Renderer` / `@tsubame/renderer-canvas` は維持する」と決めていた。本 ADR はその命名サブ決定のみを覆す**（境界・host bootstrap 退去の本筋は維持）。同 ADR には forward 注記済み。
- 関連: ADR-0002（3 層構成・命名元）、ADR-0007（`canvas-renderer-owns-viewport`・撤去済み挙動）、docs/adr/0004 §5（命名維持の旧決定・本 ADR が反転）、ADR-0012（合成ルート・本改名を前提に `@tsubame/renderer-hayate` を host adapter が new する）。
