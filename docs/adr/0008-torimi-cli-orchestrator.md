# Torimi CLI — バンドラを持たないオーケストレータと配線の畳み込み

Expo でいう `expo` コマンドに当たる層が無名のまま、example 内のスクリプト（`torimi-android-dev-server.mjs` / `torimi-dev-server.mjs`）・`vite.config.android.ts` / `vite.config.torimi.ts`・`Tsubame/scripts/lower-for-hermes.mjs` に散らばっていた。これを公開 CLI に畳む。読者が Expo からの類推で「CLI がバンドラ（metro 相当）を内蔵する」と推測しそうな箇所で、**逆**を選んでいるのが本 ADR の要点。

## 決定

1. **CLI はオーケストレータ型（バンドラ非内蔵）。** 無スコープ npm パッケージ `torimi`（bin `torimi`、設定 `torimi.config.*`）が、アプリの宣言したビルドコマンドを回し、ターゲット固有の降格（Hermes lowering は FW 固有ではなくターゲット固有＝CLI の責務、`lower-for-hermes` は CLI へ移管）を施し、`@torimi/dev-server` で配信・reload・QR まで面倒を見る。**FW もビルドツールも解さない**（ビルドコマンドは不透明な設定値）— 「dev-server は FW/ビルドツール非依存」（Torimi ADR-0001 系の原則)は下層でそのまま維持し、CLI はその上の層。Expo/metro 型（CLI が vite 設定・FW プラグインを内蔵）は、CLI に solid/react/vue の知識が生えて原則と衝突するため退けた。
2. **コマンド面は α 最小の 3 つ**: `torimi dev [target]`（既定 native）・`torimi build [target]`・`torimi lower <file>`。1 回の起動 = 1 ターゲットで、Dev Server 契約（1 サーバー = 1 bundle）とネイティブ焼き込み既定ポート（5179）は現状維持。Expo 型の 1 サーバー多ターゲット同時配信は契約とホスト双方に手が入るため α ではやらない（CLI のコマンド面はそのまま育てられる）。
3. **watch は CLI が所有（一発ビルド方式）。** config はフラットに `build`（一発コマンド）と `bundle`（出力パス）だけ。`torimi dev` はソース（既定 `src/`）変更ごとに build を再実行し、native はビルド完了後に降格して**降格済みの別パスを配信**する（未降格バンドルを配らない現行安全策を継承）。vite `--watch` 常駐方式は `torimi build` 用の non-watch 宣言が二重に要るため退けた（必要なら optional `buildWatch` を後付け）。
4. **エントリボイラープレートは `@torimi/bundle` に畳む**（Torimi CONTEXT.md「Bundle Registration」）。protocol version 焼き込み・mount seam（`__torimiMount` / `__tsubame`）・native prelude は wire 契約であり、コピペ配布は handshake 変更のたびに外部アプリを黙って壊す。アプリは `registerTorimiApp(mount: TsubameMount)` を呼ぶ**全ターゲット共通 1 エントリ**だけを書き、ターゲット差は `__hayateHost` の有無でランタイム内部分岐する。FW 知識は `TsubameMount` 引数で受け FW 盲目（依存方向は Torimi → Tsubame、CONTEXT-MAP 準拠、内部で `runTsubameApp` を呼ぶ上位層）。この結果 native/web のバンドル差は降格だけになり、vite config も torimi.config も 1 本化される。
5. **vite preset は 2 部品に分解して配る**: FW 変換は各 adapter の subpath（`@torimi/tsubame-solid/vite` / `@torimi/tsubame-react/vite`）、App Bundle 形状（単一 IIFE・es2020・DOM なし・非圧縮）は `@torimi/bundle/vite`。CLI 本体には置かない（ビルドツール非依存を維持）。
6. **スキャフォルダ `create-torimi`（`npm create torimi`）。** テンプレートの正本はモノレポ内（リリース列車に乗せ、「公開パッケージだけでビルドできるか」を CI で検証）。`create-torimi` は publish 時にテンプレートを**同梱**し（生成時に GitHub へ取りに行かない — `create-torimi@X` は必ず X 列車の雛形を吐く）、リリース CI が公開テンプレートリポジトリへ**自動ミラー**して "Use this template" とブラウズ性を満たす。生成時 fetch（degit 方式）は版数ズレ・オフライン失敗モードの分だけ不利なので退けた。

## Consequences

- example 群は最終的に `torimi.config.mjs` + 1 エントリ + preset 利用の vite config へ縮退し、`scripts/torimi-*-dev-server.mjs` と `main.android.tsx` / `main.torimi.tsx` の二重エントリは廃止方向。
- `torimi dev` の web 再ビルドは vite --watch 増分より遅くなる（α の規模では許容、`buildWatch` 後付けで回復可能）。
