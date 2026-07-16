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

## `layer-present` の実 Chromium 検証（#697）

`layer-present`（#690・ADR-0125/0127/0140）は同じ WASM を `?layerPresent=0/1` で切り替える。
`layer-present-webgpu.spec.ts` は両経路を実
Chromium（Playwright、`--enable-unsafe-webgpu --ignore-gpu-blocklist --use-angle=vulkan`）で
起動し、`navigator.gpu.requestAdapter()` の成否・`selected scene renderer` ログ・scroll
compositor の panic/device loss・優先度セグメントの interaction・フレーム遅延を検証する。
WebGPU canvas の画素パリティは `hayate-scene-renderer-vello` の GPU readback tests が担当する。

**本番の `pnpm test:e2e` には含まれない**（既定ビルド `wasm-pkgs/pkg` しか無い環境でも他の
スモークを止めずに走らせるため、専用の config/script に分離してある）。

```bash
# Tsubame/examples/todo で（標準 pkg の再ビルド込み）
pnpm test:e2e:layer-present
```

- 標準ビルド `Hayate/wasm-pkgs/pkg` を1つの dev server で配信する（既定ポート 5185、
  `E2E_LAYER_PRESENT_PORT` で変更可）。
- WebGPU アダプタが取れない環境では `test.skip` で理由を明示してテスト出力に残す
  （黙って green にはしない）。
- Playwright 管理の chromium が未インストールの環境では `/opt/pw-browsers/chromium` →
  システムの `google-chrome`（`/usr/bin/google-chrome`）の順にフォールバックする
  （`playwright.config.layer-present.ts`）。

## CanvasKit performance feedback loop（#832–#834）

共有 Task Studio fixture を実 Chromium + CanvasKit WebGL surface で駆動し、静止、text editing、
scroll、theme transition animation を同じ harness で計測する。

```bash
# 通常 CI / agent 実行。WASM を先に再ビルドし、環境差を許容する baseline contract を判定する。
pnpm test:e2e:canvaskit-perf

# dirty-layer replay と allocation のさらに厳しい改善目標。
pnpm test:e2e:canvaskit-perf:strict
```

各 scenario は frame replay 時間、full-scene / dirty-layer replay 回数、composite-only frame
回数、command payload bytes、Paint / Font / scratch / command-decode allocation 回数、WebGL
version / renderer / software 判定を JSON attachment と標準出力へ記録する。通常 contract は
editing / scroll / animation の full-scene replay を 0 に固定する。端末依存の frame time は
レポート専用で閾値判定せず、CI は `CANVASKIT_PERFORMANCE_BUDGET` の replay/allocation contract
だけを判定する。厳格モードは
`CANVASKIT_STRICT_RED_BUDGET` を使い、hardware 固有の数値較正は #816 に委ねる。
