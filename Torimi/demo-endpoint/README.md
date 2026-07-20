# @torimi/demo-endpoint

Torimi の **Demo Endpoint**（ADR-0003）。ビルド済みデモ App Bundle と Demo Manifest
（`/demos.json`）を常時 HTTPS 配信する Cloudflare Worker。Dev Server と違い watch もビルドも
行わず、`/reload` への WS は受けて保持するだけで reload は送らない。

## ローカル起動

```sh
pnpm --filter @torimi/demo-endpoint run build:demos   # solid / react の torimi:native:build → public/
pnpm --filter @torimi/demo-endpoint run dev           # wrangler dev
```

デモ一覧の正本は `src/demos.json`。`name` / `bundleUrl` は wire（Demo Manifest）へ、`source` は
`build:demos`（どのパッケージの Android バンドルをどこから copy するか）へ流れる。

## デプロイ（リリース lockstep・ADR-0003）

本番デプロイは **リリース lockstep CI**（`.github/workflows/torimi-release.yml`）の領分。
`torimi-android-v*` タグの push か手動実行（workflow_dispatch）だけがトリガで、main への
push ではデプロイしない — Play 配布済みホストに焼き込まれた Protocol Version とデモバンドルの
整合を構造で守るため、ホスト（将来の AAB）とデモは常に同じタグ付きコミットから作る。

`pnpm run deploy` は手動実行時にも古い `public/` を公開しないよう、Manifest に載る全デモの
`build:demos` を必ず先に実行する。`wrangler deploy` の直接実行は生成物のProtocol Versionを
更新しないため使用しない。

### 必要な GitHub Secrets（登録は人力）

CI の wrangler deploy は次の 2 つの Secrets を要する。登録作業そのものは完全人力スライスの
領分（エージェントは資格情報に触れない）。

| Secret 名 | 値 |
| --- | --- |
| `CLOUDFLARE_API_TOKEN` | **Workers 編集権限に絞った** API トークン |
| `CLOUDFLARE_ACCOUNT_ID` | 対象 Cloudflare アカウントの Account ID |

登録手順:

1. **API トークンを作る** — Cloudflare ダッシュボード → My Profile → API Tokens →
   Create Token → テンプレート **Edit Cloudflare Workers** を選ぶ（権限を Workers Scripts の
   編集に絞る。Global API Key は使わない）。対象アカウント／ゾーンをこのアカウントに限定して発行し、
   表示されたトークン値を控える。
2. **Account ID を控える** — ダッシュボードの Workers & Pages 概要ページ右側（または任意の
   ゾーンの Overview 右下）に表示される Account ID をコピーする。
3. **GitHub に登録する** — リポジトリの Settings → **Secrets and variables** → Actions →
   New repository secret で、上表の名前どおりに 2 件登録する。
4. **動作確認** — Actions タブから `Torimi release lockstep` を workflow_dispatch で手動実行し、
   wrangler deploy が通ることを確認する（`https://<worker>.workers.dev/demos.json` が応答すれば成功）。
