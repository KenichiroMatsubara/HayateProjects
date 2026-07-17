# npm α配布 — 公開クロージャ・lockstep 列車・CI-only publish

Tsubame（≈ React Native）と Torimi（≈ Expo Go）を外部開発者が使えるようにするため、npm への α 配布を始める。後方互換は保証しない（0.x で breaking 上等）が、README を頼りにモノレポ外で dev ループが回る体裁を目指す。受け入れ条件は「モノレポ外の空ディレクトリで、公開パッケージ + README だけを頼りに `torimi dev` が動く」。

## 改訂（2026-07）— 公開スコープを単一 `@torimi` へ統合

当初案は `@hayate/*` / `@tsubame/*` / `@torimi/*` の三スコープ＋無スコープ `torimi` を前提にしていたが、公開直前の実測で**前提が崩れた**ため、公開名前空間を **`@torimi` 単一スコープ**へ統合する。

- **`@hayate` / `@tsubame` スコープは取得不可。** 無スコープ `hayate`（npm ユーザー `refulk` が create-next-app の残骸を 2024 に公開）・`tsubame`（0 version の孤立レコード、2022）が既に存在し、npm では org 名が既存の無スコープ package 名と衝突するため、org `hayate` / `tsubame`（＝スコープ `@hayate` / `@tsubame`）を作成できない。
- **無スコープ `torimi` と `@torimi/*` は同居できない。** org `torimi` を作ると無スコープ `torimi` は publish 不可になる（同一名前空間）。当初案の「`@torimi/bundle` 等＋無スコープ `torimi` CLI」は原理的に両立しなかった。
- **決定：全公開パッケージを `@torimi/*` に寄せる。** `@hayate/host` → `@torimi/hayate-host`、`@tsubame/solid` → `@torimi/tsubame-solid` のように、旧スコープ名を leaf の接頭辞として残す。CLI は `@torimi/cli` に改名し、コマンド名は `bin` で `torimi` を維持（利用者は従来どおり `torimi dev`）。`create-torimi` は別名のため無スコープのまま残し、`npm create torimi` も従来どおり動く。
- **必要な npm org は `torimi` の 1 つだけ。** `NPM_TOKEN` の権限も `@torimi` スコープ＋無スコープ `create-torimi` で足りる。
- **奪還はブロッカーにしない。** `hayate` / `tsubame` は一般語で商標主張が弱く、`refulk` への移譲依頼／npm support への孤立名解放申請は非同期の宿題とする。取得できた場合は別名エイリアスとして後付け可能。

以下の元決定のうち**スコープ命名に関する記述はこの改訂で上書きされる**（lockstep 列車・CI-only publish・スタブ検査などの仕組みは不変）。

## 決定

1. **公開範囲は外部アプリの依存クロージャ全部。** `@tsubame/*`（app / solid / react / renderer-hayate / renderer-dom / renderer-protocol / protocol-generated / hayate-css-catalog）、`@hayate/*`（host / protocol-spec / wasm adapter 群）、`@torimi/*`（dev-server / dev-server-contract / host-web / protocol-handshake / bundle）＋ 無スコープ `torimi`（CLI, ADR-0008）・`create-torimi`。`@torimi/hayate-host` は App（合成ルート）の直接依存なので隠せない（docs/adr/0004 の帰結）。examples・`@torimi/integration`・`@torimi/demo-endpoint`・`hayate-fonts`・`pkg-null` は非公開のまま。Hayate 本体（Rust）と Play 配布（Torimi ホスト APK）は npm と別世界でスコープ外。
2. **wasm パッケージはスコープ入りへ改名**（`hayate-adapter-web` → `@torimi/hayate-adapter-web` 等）。無スコープ `hayate` / `tsubame` は他人に取られており（`torimi` は空きで CLI が確保）、無スコープ名を散らかさない。`@torimi/hayate-host` の `file:../wasm-pkgs/pkg` 依存は publish で壊れるため `workspace:*` → version 置換へ変更する。backend 選択（Vello / tiny-skia）は host の機能なので wasm を host に同梱せず個別公開する。
3. **全公開パッケージ lockstep（固定バージョン列車、0.1.0 から）。** wire 契約が `@torimi/hayate-protocol-spec` → `@torimi/tsubame-protocol-generated` → `@torimi/tsubame-renderer-hayate`（PROTOCOL_VERSION）→ Torimi ホスト decoder とパッケージ横断で鎖になっており、independent 版数は互換マトリクス管理を生む — 後方互換を捨てる α と最悪の相性。案内も「全部同じ版数に揃えろ」の一文で済み、Torimi ホスト（Play）との整合も「ホスト vX ⇔ 0.x 列車」と 1:1 で言える（Expo SDK 整合と同型）。changesets の `fixed` グループで実装する。
4. **publish は GitHub Actions のリリースワークフロー一本。手元 publish 禁止。** changesets の version PR を merge したら CI が Rust → wasm ビルド → JS 全ビルド → スタブ検査 → `pnpm -r publish`（npm provenance 付き）まで回す。wasm は fresh clone で `bootstrap-wasm-pkgs.mjs` がスタブを置く仕組みのため、**スタブを publish する事故は機械検査で塞ぐ**（npm は unpublish 制限が強く、事故が消せない）。dist-tag は `latest`（α であることは 0.x が語る）。
5. `@torimi/tsubame-protocol-generated` と `@torimi/tsubame-hayate-css-catalog` は `.ts` 直 export のため、publish 前に dist ビルドを吐く形へ変更する。α の adapter は solid / react のみ（vue は未実装）。

## 消極的決定（やらないこと）

- semver 互換保証・deprecation ポリシー（α では負わない）
- `alpha` dist-tag 運用（0.x で足りる）
- Demo Endpoint / Play 配布パイプラインの変更
