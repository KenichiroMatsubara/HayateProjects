# E2E（実ブラウザでの動作確認）

AI / 人間が **本物の Chromium** でアプリを起動して挙動を確認するための Playwright ハーネス。

`vitest + happy-dom`（`src/**/*.test.ts`）は擬似 DOM のユニットテスト。こちらは
本物のブラウザ・本物のレイアウト・スクリーンショットを使う E2E で、役割が違う。

## 使い方

```bash
# Tsubame/examples/todo で
pnpm test:e2e            # ヘッドレスで全 spec を実行（vite dev は自動起動）
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

## レンダラーの選択

アプリは DOM / canvas(vello,tiny-skia) を切り替えられる（`?renderer=` クエリ）。

- `?renderer=dom` … WebGPU/WASM 不要。CI・ヘッドレスのスモークはこれ。
- `?renderer=vello` / `?renderer=tiny-skia` … canvas 実描画。`wasm-pkgs` のビルドと
  GPU 環境が要るので、確認は screenshot / `toHaveScreenshot` で行う。

## spec を足すときの指針

- 安定セレクタ: 追加フォームは `input[placeholder="新しいタスクを入力…"]`、
  seed タスクの文言は `todo-model.ts` の `SEED`。
- canvas レンダラーは DOM を覗けないので、`expect(page).toHaveScreenshot()` で確認する。
  Accessibility Mirror（ADR-0124、`[data-hayate-a11y]`）経由で `getByRole` 照会 →
  `boundingBox()` の座標で canvas をクリックする駆動パターンは `canvas-a11y-mirror.spec.ts` 参照。

## `layer-present` feature の実 Chromium 検証（#697）

> ⚠️ **ADR-0135 により `layer-present` feature 自体が封印中（有効化禁止）**。#697 の実
> Chromium 検証で描画バグが確認され、実用段階にないと判定された。以下の harness は
> 削除せず維持するが、再開（性能上の実害が具体的に発生した時）までは実行対象として
> 使わない — 再開時の回帰ガード／出発点として保存してある。

`layer-present`（#690・ADR-0125/0127、既定 OFF）は cargo feature なのでランタイムに切り替え
不可 — ON/OFF は別 WASM バイナリになる。`layer-present-webgpu.spec.ts` はこの2ビルドを実
Chromium（Playwright、`--enable-unsafe-webgpu --ignore-gpu-blocklist --use-angle=vulkan`）で
起動し、`navigator.gpu.requestAdapter()` の成否・`selected scene renderer` ログ・優先度
セグメントトグル後の canvas 画素一致・クリック→フレームのレイテンシ p50/p95 を記録する。

**本番の `pnpm test:e2e` には含まれない**（既定ビルド `wasm-pkgs/pkg` しか無い環境でも他の
スモークを止めずに走らせるため、専用の config/script に分離してある）。

```bash
# 1. ON 版 WASM ビルド（Hayate/wasm-pkgs/pkg-layer-present、default features + layer-present）
pnpm --filter hayate build:layer-present

# 2. Tsubame/examples/todo で
pnpm test:e2e:layer-present
```

- OFF は既定ビルド（`Hayate/wasm-pkgs/pkg`、`vite.config.ts` そのまま）、ON は
  `vite.config.e2e-layer-present.ts`（`hayate-adapter-web` を `pkg-layer-present` へ alias、
  本番コードは無変更）で配信する別 dev server（既定ポート 5185/5186、`E2E_LAYER_PRESENT_OFF_PORT`
  / `E2E_LAYER_PRESENT_ON_PORT` で変更可）。
- WebGPU アダプタが取れない環境では `test.skip` で理由を明示してテスト出力に残す
  （黙って green にはしない）。
- Playwright 管理の chromium が未インストールの環境では `/opt/pw-browsers/chromium` →
  システムの `google-chrome`（`/usr/bin/google-chrome`）の順にフォールバックする
  （`playwright.config.layer-present.ts`）。
