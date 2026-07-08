# npm α配布 — 公開クロージャ・lockstep 列車・CI-only publish

Tsubame（≈ React Native）と Torimi（≈ Expo Go）を外部開発者が使えるようにするため、npm への α 配布を始める。後方互換は保証しない（0.x で breaking 上等）が、README を頼りにモノレポ外で dev ループが回る体裁を目指す。受け入れ条件は「モノレポ外の空ディレクトリで、公開パッケージ + README だけを頼りに `torimi dev` が動く」。

## 決定

1. **公開範囲は外部アプリの依存クロージャ全部。** `@tsubame/*`（app / solid / react / renderer-hayate / renderer-dom / renderer-protocol / protocol-generated / hayate-css-catalog）、`@hayate/*`（host / protocol-spec / wasm adapter 群）、`@torimi/*`（dev-server / dev-server-contract / host-web / protocol-handshake / bundle）＋ 無スコープ `torimi`（CLI, ADR-0008）・`create-torimi`。`@hayate/host` は App（合成ルート）の直接依存なので隠せない（docs/adr/0004 の帰結）。examples・`@torimi/integration`・`@torimi/demo-endpoint`・`hayate-fonts`・`pkg-null` は非公開のまま。Hayate 本体（Rust）と Play 配布（Torimi ホスト APK）は npm と別世界でスコープ外。
2. **wasm パッケージはスコープ入りへ改名**（`hayate-adapter-web` → `@hayate/adapter-web` 等）。無スコープ `hayate` / `tsubame` は他人に取られており（`torimi` は空きで CLI が確保）、無スコープ名を散らかさない。`@hayate/host` の `file:../wasm-pkgs/pkg` 依存は publish で壊れるため `workspace:*` → version 置換へ変更する。backend 選択（default / tiny-skia / vello-cpu）は host の機能なので wasm を host に同梱せず個別公開する。
3. **全公開パッケージ lockstep（固定バージョン列車、0.1.0 から）。** wire 契約が `@hayate/protocol-spec` → `@tsubame/protocol-generated` → `@tsubame/renderer-hayate`（PROTOCOL_VERSION）→ Torimi ホスト decoder とパッケージ横断で鎖になっており、independent 版数は互換マトリクス管理を生む — 後方互換を捨てる α と最悪の相性。案内も「全部同じ版数に揃えろ」の一文で済み、Torimi ホスト（Play）との整合も「ホスト vX ⇔ 0.x 列車」と 1:1 で言える（Expo SDK 整合と同型）。changesets の `fixed` グループで実装する。
4. **publish は GitHub Actions のリリースワークフロー一本。手元 publish 禁止。** changesets の version PR を merge したら CI が Rust → wasm ビルド → JS 全ビルド → スタブ検査 → `pnpm -r publish`（npm provenance 付き）まで回す。wasm は fresh clone で `bootstrap-wasm-pkgs.mjs` がスタブを置く仕組みのため、**スタブを publish する事故は機械検査で塞ぐ**（npm は unpublish 制限が強く、事故が消せない）。dist-tag は `latest`（α であることは 0.x が語る）。
5. `@tsubame/protocol-generated` と `@tsubame/hayate-css-catalog` は `.ts` 直 export のため、publish 前に dist ビルドを吐く形へ変更する。α の adapter は solid / react のみ（vue は未実装）。

## 消極的決定（やらないこと）

- semver 互換保証・deprecation ポリシー（α では負わない）
- `alpha` dist-tag 運用（0.x で足りる）
- Demo Endpoint / Play 配布パイプラインの変更
