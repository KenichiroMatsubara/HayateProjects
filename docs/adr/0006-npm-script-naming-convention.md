# npm script は「コンテキスト:バリアント:動詞」で命名し、バリアント軸はコマンドが実際に分岐する軸に一致させる

**Status: accepted**

**Date: 2026-07-08**

## Context

モノレポの npm script 名が 3 つの文法（`torimi:android:dev`＝対象:バリアント:動詞、`dev:hayate`＝動詞:対象、`android:build`＝バリアント:動詞）で混在し、加えて実体との不整合が溜まっていた。

- `dev:hayate` は `pnpm --filter hayate dev` を指すが、Hayate に `dev` script は一度も存在せず **dead entry** だった。
- root の `torimi:android:dev` は委譲先 todo で `torimi:android:serve` に**名前が変わり**、`dev`／`serve` の同義語ペアを生んでいた。
- `torimi:android:*`／`build:android`／`build:torimi` の `android` は、実際には「Android という OS」ではなく「**Hermes 用に降格済みバンドルを食う Native Host か、es2020 のまま食う Web Host か**」というホスト種別を指していた（Torimi CONTEXT.md の Host／Web Host）。バンドルを 2 種に分けている軸は OS ではなくホスト種別である。

## Decision

npm script 名の文法を **`コンテキスト:バリアント:動詞`** に統一する。

- **コンテキスト**（第 1 セグメント）は境界づけられたコンテキスト名（`hayate` / `torimi` / …）。CONTEXT-MAP のコンテキストと同型にし、「どの CONTEXT.md の語彙で動くか」を名前で示す。
- **バリアント**（中間セグメント）は**そのコマンドが実際に分岐する軸**を使う。dev / build はホスト種別なので `native` / `web`、install は OS 固有（gradle `installDebug`）なので `android`。dev が `native` で install が `android` という「軸の混在」は不整合ではなく、各コマンドが本当に分岐する軸をそのまま名乗った結果である。
- **動詞**（最終セグメント）は閉じた 4 語彙に限る: `dev`（watch して配信し続けるループ）/ `build`（一発ビルド）/ `run`（一発ビルド＋起動）/ `install`。`serve` は `dev` の同義語として廃止。

補助規則:

- **コンテキスト接頭辞の義務条件**: 「root から委譲される入口」または「ホストパッケージと異なるコンテキストに属する script」にのみ接頭辞を付ける。ホストパッケージ＝コンテキストの内部作業（Hayate 内の `android:build` 等）は接頭辞が冗長なので付けない。これにより「なぜ `torimi:android:install` が Hayate パッケージに接頭辞付きで居るのか」（＝Hayate に置かれた Torimi コンテキストの入口だから）が一貫して説明できる。
- **委譲チェーンは全レベル同名**: root の `torimi:native:dev` は委譲先でも `torimi:native:dev`。`grep` 一発で定義と実体の両方に届く。
- **モノレポ全体への一括操作**（`build` / `test` / `typecheck` / `clean` / `dev` / `check:proto`）は裸の動詞のまま。これは特定コンテキストではなく全ワークスペースを対象にする別カテゴリ。

## Considered Options

- **すべて `native` に寄せる**（`torimi:native:install`）: install は gradle `installDebug` そのもので、iOS 対応時に別コマンド（`xcrun` 等）へ確実に割れる。`native` を名乗ると「Android しか入れられないのに native」という嘘を名前に含むため却下。
- **現状維持で全部 `android`**: バンドルを分ける真の軸（ホスト種別）を名前が隠し続ける。iOS Native Host 着手時にまとめて改名するコストは、軸を今正す利益に劣ると判断し却下。

## Consequences

- root: `dev:hayate`（dead）を削除し `hayate:desktop:run` / `hayate:desktop:build` を新設。`torimi:android:dev`→`torimi:native:dev`、`torimi:web:dev` を新設。`torimi:android:install` は据え置き。
- 委譲先: todo/react-demo の `build:android`→`torimi:native:build`、`build:torimi`→`torimi:web:build`。todo の `torimi:android:serve`→`torimi:native:dev`、`torimi:web:dev` 新設。Hayate の `desktop`／`desktop:build`→`hayate:desktop:run`／`hayate:desktop:build`。
- 参照追随: `build-demos.mjs`・`torimi-android-dev-server.mjs` の spawn 等、旧 script 名を叩く機能参照を更新。CI／README／コメントも追随。
- **dev server の .mjs ファイル名**（`torimi-android-dev-server.mjs` / `torimi-dev-server.mjs`）は据え置き。E2E の起動経路（playwright.config.ts の node 直叩き）を変えない判断で、本 ADR のスコープ（script 名）外。ファイル名と script 名の軽微な不一致は許容し、将来 E2E 経路を触るときにまとめて解消しうる。
- 既存 ADR 本文（例: Torimi ADR-0003 の `build:android` 参照）は決定当時の記録として書き換えない。現在の script 名の正本は本 ADR。
