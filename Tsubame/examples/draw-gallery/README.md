# Tsubame Draw Gallery

draw 機能 v1（PRD #723 / ADR-0141）の集大成デモ（issue #732）。draw v1 語彙を横断する
サンプル painter を、`view` の `draw` property に載せて敷き詰め、**同一 painter を Hayate
Renderer / DOM Renderer の両経路で表示**する。

painter はフレームワーク非依存・レンダラー非依存の純関数（`src/painters.ts`）。同じ関数が
Hayate 経路（wire の `draws` チャネル → WASM ラスタライザ）でも DOM 経路（各 `view` に敷いた
`<canvas>` への canvas 2D replay・Tsubame ADR-0014）でも同じ絵を出す。両経路のピクセル一致は
要求しない（形状の目視同等のみ）。

## サンプル painter（5 種）

| id                | 実証する draw v1 語彙                              |
| ----------------- | ------------------------------------------------- |
| `curve-chart`     | cubic bezier（3 次ベジェの折れ線チャート）        |
| `even-odd-donut`  | evenOdd 塗り規則（中抜き円）                       |
| `dash-sampler`    | 破線 + 線端(cap)/角(join) の見本                   |
| `rotated-clip`    | 回転 + クリップの組み合わせ                        |
| `responsive-grid` | サイズ追従（box が広がるとセル数が増える市松模様）|

`responsive-grid` は resize→paint ループの実地デモも兼ねる（画面下のボタンで box 寸法を
変えると painter が別の絵を描き直す）。

## 起動

```sh
pnpm --filter @tsubame/example-draw-gallery dev
```

レンダラは右上のトグル、または URL クエリで切り替える:

- `?renderer=dom` — DOM Renderer（canvas 2D replay、WebGPU/WASM 不要）
- `?renderer=tiny-skia` — Hayate Renderer / CPU バックエンド（WebGPU 無しでも可）
- `?renderer=vello` — Hayate Renderer / WebGPU バックエンド
- `?renderer=auto`（既定）— 環境から自動選択

tiny-skia / vello 経路は WASM ビルドが要る: `pnpm --filter hayate build`。

## テスト

```sh
pnpm --filter @tsubame/example-draw-gallery test       # painter のユニットテスト（vitest）
pnpm --filter @tsubame/example-draw-gallery test:e2e   # Playwright e2e（両レンダラー経路）
```

- `e2e/dom.spec.ts` — DOM 経路。全 draw canvas が空白でないこと、サイズ追従で描き直すことを検証。
- `e2e/hayate.spec.ts` — Hayate 経路（tiny-skia）。WASM 未ビルド / バックエンド不可の環境では
  理由付きで skip する。
