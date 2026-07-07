# Demo Endpoint はリリース lockstep でデプロイし、デモ一覧は Demo Manifest が駆動する

> **用語更新（ADR-0004・2026-07-07）**: 本 ADR が言及する "Miharashi" は **Torimi（鳥見）** に改名された（ディレクトリ・`@torimi/*` パッケージスコープ・wire グローバル・タグ規約 `miharashi-android-v*` → `torimi-android-v*`・Worker 名等の全面リネーム）。本文は決定当時の記録として原文のまま。

status: accepted

Date: 2026-07-06

## Context

Google Play 公開（テスター・審査者への配布）には、開発機の稼働と無関係に「いつでも」応答するデモ配信点が
要る。Play 配布ホストには decoder の Protocol Version が焼き込まれ、バンドル側版数と不一致なら明示エラーに
なる（ADR-0001）ため、配信点上のデモバンドルは**配布済みホストと版数が一致し続け**なければならない。
またデモは solid だけでなく react（将来 vue）も同一ホストで選べる形を目指す——「Viewer 一本で全 JS FW が
動く」（ADR-0001）をデモ体験にもそのまま貫くためである。

## Decision

- **Demo Endpoint は Cloudflare Workers（静的アセット + 最小 Worker）**。各デモバンドルを静的アセットで
  HTTPS 配信し、`/reload` は WS 接続を受けて黙って保持する（ホストの 1 秒 backoff 再接続が無意味な通信を
  打ち続けないための受け皿。reload は送らない）。URL は workers.dev サブドメインで開始。
- **デプロイは Play リリースと lockstep**。GitHub Actions がリリースタグ（`miharashi-android-v*`）と
  `workflow_dispatch` だけをトリガに、**AAB と同じコミット**からデモバンドルをビルドして wrangler で
  デプロイする。main への push ではデプロイしない。ホストとバンドルが常に同一コミット由来になるため、
  Protocol Version 不一致が「注意」ではなく「構造」で防がれる。
- **デモ一覧は Demo Manifest（`/demos.json`）が駆動する**。ホストは起動時にマニフェストを取得してデモ選択
  メニュー（表示名 + バンドル URL）を構成し、初回起動は先頭デモを自動ロードする（ゼロ入力で動く）。
  デモの追加・改名はマニフェスト更新であり、Play 審査を要しない。
- **接続先はフル URL になる**。複数デモを同一ホスト名で区別するため、target は従来の `host:port`（path 破棄）
  からフル URL（path 保持）へ広がる。ADR-0002 の OS スタック委譲と同じ変更に乗せる。

## Considered Options

- **main への push ごとにデモをデプロイ**：main 上の Protocol Version 更新が、Play 上の全テスターを一斉に
  明示エラーへ落とす。デモ配信点は main の速度ではなく Play リリースの速度で動くべき。却下。
- **デモ一覧をホストへハードコード**：デモを 1 つ足すたびに Play リリースと審査が要る。「ホストは再ビルド
  せず、バンドルだけ差し替える」という Miharashi の存在理由に反する。却下。
- **Cloudflare Tunnel で開発機の dev-server を公開**：「devコマンドで配信されるものそのもの」だが、開発機が
  落ちていれば配信も止まり「いつでも」に反する。テスター・審査向けには不成立。個人のリモート開発用途として
  将来併用は可能。却下（本目的では）。

## Consequences

- タグを打つ 1 アクションが Play（AAB）と Cloudflare（デモ）の両方の起点になり、リリース間は配信点が凍結される。
- 旧ホストを掴んだままのテスターとの互換が将来問題になったら、Protocol Version ごとに Worker サブドメインを
  分ける拡張（例: `miharashi-demo-p3.…workers.dev`）で対処できる（初回は 1 endpoint）。
- react デモを載せるには react-todo に Android 用ビルド（Hermes 降格つき `build:android` 相当）の配線が要る
  （solid 版の `vite.config.android.ts` + `lower-for-hermes` を写す）。「react もできる設計」を初回スコープに含める。
- デモ再デプロイ時に接続中テスターへ `reload` を一斉送信する拡張（Durable Objects）は同じ席に載るが、
  初回スコープには含めない（アプリ再起動で足りる）。
