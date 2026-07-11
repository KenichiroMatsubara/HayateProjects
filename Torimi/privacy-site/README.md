# @torimi/privacy-site

Torimi の**プライバシーポリシー**を常時 HTTPS 配信する Cloudflare Worker（アセットのみ）。
Google Play ストア掲載に必須の「プライバシーポリシー URL」を提供する。

本文は [`public/index.html`](public/index.html)。日本語・単一ページ・自己完結（外部 CSS/JS/フォント依存なし）。

## ローカル確認

```sh
pnpm --filter @torimi/privacy-site run dev   # wrangler dev（http://localhost:8787 で確認）
```

## デプロイ

```sh
pnpm --filter @torimi/privacy-site run deploy   # wrangler deploy
```

デプロイ先は Cloudflare アカウント `pinara`（`kenmatsu331@gmail.com`）。公開 URL は
`https://torimi-privacy.<subdomain>.workers.dev/`。このルート URL をそのまま Play Console の
「プライバシーポリシー」欄に登録する。

> 本文を更新したら `public/index.html` を編集して再 deploy するだけでよい（アプリ更新・Play 審査は不要）。
