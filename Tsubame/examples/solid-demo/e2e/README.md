# E2E（実ブラウザでの動作確認）

AI / 人間が **本物の Chromium** でアプリを起動して挙動を確認するための Playwright ハーネス。

`vitest + happy-dom`（`src/**/*.test.ts`）は擬似 DOM のユニットテスト。こちらは
本物のブラウザ・本物のレイアウト・スクリーンショットを使う E2E で、役割が違う。

## 使い方

```bash
# Tsubame/examples/solid-demo で
pnpm test:e2e            # ヘッドレスで全 spec を実行（vite dev は自動起動）
pnpm test:e2e:worker     # DPR=3 の標準 Canvas Worker workload
pnpm test:e2e -- smoke   # spec を絞る
pnpm test:e2e:ui         # Playwright UI モード（人間向け・要 GUI）
pnpm test:e2e:report     # 直近の HTML レポートを開く
```

`webServer` が `vite` を自動で立ち上げる（既定ポート 5180、`E2E_PORT` で変更可）。
すでに dev サーバーが動いていれば再利用する。

## 初回だけ必要なセットアップ

```bash
pnpm exec playwright install chromium        # ブラウザ本体（ダウンロード済みなら不要）
sudo pnpm exec playwright install-deps chromium   # Linux のシステムライブラリ（要 root）
```

WSL2 / 素の Linux では `libnss3 libnspr4 libasound2` 等が無いと Chromium が起動できない
（`exitCode=127`）。上の `install-deps` が distro に応じて入れてくれる。

## Web 表示モード

アプリは DOM / Canvas を切り替えられる（`?renderer=` クエリ）。

- `?renderer=dom` … WebGPU/WASM 不要。CI・ヘッドレスのスモークはこれ。
- `?renderer=auto`（既定）… OffscreenCanvas＋単一 Worker の標準 Canvas 経路。renderer は
  Worker 内の Render Host が初回 boot 時に選び、query flag では強制しない。

`test:e2e:worker` は 320×568 / DPR 3、warmup 1 回、sample 5 回×50 move、60 Hz
frame budget 16.67 ms の名前付き touch-scroll workload を実行する。Linux headless では
Chrome を GPU/Vulkan 有効で起動する。

## spec を足すときの指針

- 安定セレクタ: 追加フォームは `input[placeholder="新しいタスクを入力…"]`、
  seed タスクの文言は `todo-model.ts` の `SEED`。
- Canvas は DOM を覗けないので、pixels と Worker の共通 pipeline observation を併用する。
