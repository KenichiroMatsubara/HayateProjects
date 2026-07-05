# フォント配信をjsDelivrから自前ホスト（Cloudflare R2 + Worker）へ移行

## Context

`fonts.json`（[web/fonts.json](../../crates/platform/web/fonts.json)）は29フォント全てを
jsDelivrのGitHubミラー経路（`cdn.jsdelivr.net/gh/google/fonts@main/...`）から取得していた。
ADR-0106でCore/アダプタ双方にリトライ・指数バックオフを実装済みだったが、それでも
「ありえないほど失敗する」という報告があった。

調査の結果、2つの異なる問題が混在していたと判明した：

1. **恒久的な失敗（リトライで直らない）**：`Noto Color Emoji`
   （`NotoColorEmoji-Regular.ttf`, 24.27 MB）は**常に**HTTP 403を返していた。
   レスポンスボディは `File size exceeded the configured limit of 20 MB.` —
   jsDelivrのGitHubミラー経路にはハードな20MBファイルサイズ上限があり、この
   ファイルは恒久的に配信不可能。ADR-0106のリトライ機構は一時障害を前提にしており、
   この種の恒久403には無力。
2. **一時的な失敗（jsDelivr側の可用性）**：残り28フォントは調査時点では全て200 OKだったが、
   GitHub Fontsリポジトリ更新中の一時的な404やレート制限は past issue（#343）でも
   報告されており、依然として不確実性が残る。

なお「GH ActionsのCI/デプロイ失敗の大半がFontFetch起因」という当初の仮説は、直近の
失敗履歴を確認したところ根拠がなかった（`hayate-adapter-web-layer-present` /
`hayate-adapter-web-vello-cpu` のTS型解決エラーで、無関係の既知の問題）。今回の対応は
本番ランタイムでのフォント配信信頼性のみを対象とする。

## 決定

### 1. 全29フォントをCloudflareの自前CDNへ完全移行し、jsDelivrへのフォールバックは持たない

jsDelivrをフォールバック先として残す案もあったが、絵文字フォントのケースのように
恒久403の対象にはフォールバックしても意味がなく、複雑さだけが増える。既存の
`FontFetchTracker`（[font_fetch.rs](../../crates/core/src/element/font_fetch.rs)）の
リトライ予算・バックオフはそのまま流用し、向き先のURLだけを差し替える。

### 2. 構成はR2（非公開バケット）+ 薄いCloudflare Worker、公開先は無料の`*.workers.dev`

このプロジェクト用の新規リポジトリ `fonts/`（pnpmワークスペースパッケージ
`hayate-fonts`）を作成した：

- `fonts/wrangler.toml` — Workerの設定とR2バインディング（`FONTS_BUCKET`）
- `fonts/worker/src/index.ts` — リクエストパスをR2キーとして1:1で引き、
  `Content-Type` / `Access-Control-Allow-Origin: *` / 長期`Cache-Control`
  （フォントは`max-age=31536000, immutable`）を付けて返すだけの薄いプロキシ
- `fonts/manifest.json` — family / R2キー / 取得元（`raw.githubusercontent.com`,
  jsDelivrと違い20MB上限なし）/ OFL.txtの取得元を記録した唯一の情報源
- `fonts/scripts/upload.mjs` — manifestから全ファイル+OFL.txtをダウンロードし
  `wrangler r2 object put` でアップロード
- `fonts/scripts/verify.mjs` — デプロイ後のWorker URLに対して全オブジェクトの
  フェッチ確認（`fonts.json`反映前のセーフティチェック）
- `fonts/scripts/generate-fonts-json.mjs` — 確認後、`web/fonts.json`をWorkerの
  ベースURLで再生成する

パブリックバケット直配信（`r2.dev`）とCloudflare Pagesは却下した（次項）。独自ドメインは
持っていないプロジェクトのため、Cloudflare公式に「個人・趣味プロジェクト向け」と
明記されている無料の`*.workers.dev`サブドメインで公開する。

### 3. フォントは固定スナップショット。自動追従の同期パイプラインは組まない

アップロード後は手動で気づいたときに再アップロードする運用とし、GitHub Fontsの
更新を自動追跡する仕組みは作らない。Workerのデプロイも手動`wrangler deploy`とし、
CI（GitHub Actions）にCloudflare API tokenを登録する自動デプロイは組まない。
更新頻度が低い静的アセットに対して、その管理コストは見合わない。

### 4. OFLライセンスの同梱

各フォントファイルと同じR2キー空間に対応する`OFL.txt`も配置する（ユーザー向け配信は
不要、再配布者としての法的な体裁を満たす目的）。

## 却下した代替案

- **`r2.dev`パブリックバケット直配信**：Cloudflare公式ドキュメントに
  「開発用途限定・レート制限あり」「本番運用は非対応（unsupported access path）」と
  明記されている。まさに今回解消したい「配信の信頼性」問題をホスティング先を変えた
  だけで再現しかねないため却下。
- **Cloudflare Pages**：静的サイトのビルド成果物として扱う設計であり、更新頻度の低い
  バイナリアセットの配信という用途とは責務が合わない。
- **段階的ロールアウト（絵文字フォント1つだけ先行移行）**：安全性は高いが、
  アップロード後にfetch確認するステップを別途設けるため、全フォント同時移行でも
  リスクは十分に抑えられると判断し、単純さを優先した。

## 影響

- 新規: `fonts/`（`wrangler.toml`, `worker/src/index.ts`, `manifest.json`,
  `scripts/upload.mjs`, `scripts/verify.mjs`, `scripts/generate-fonts-json.mjs`,
  `README.md`）
- `pnpm-workspace.yaml`：`fonts`パッケージを追加
- `.gitignore`：`.wrangler/`を追加
- `Hayate/crates/platform/web/fonts.json`：Cloudflare Worker URLへの実移行は
  Cloudflareアカウント作成・R2バケット作成・`wrangler login`（手動、ローカル環境）が
  前提のため未実施。`fonts/README.md`のセットアップ手順を参照。
