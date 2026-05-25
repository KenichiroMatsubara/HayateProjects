# Hayate（疾風）

> **Hayate は、アプリケーション UI のための命令型・保持型・GPU ネイティブな UI 基盤である。**

Hayate は UI フレームワークではない。状態管理でもない。Reconciler でもない。Component tree でもない。

Hayate が提供するのは **Element Layer**（element tree + Hayate CSS スタイル解決）と **Raw Layer**（絶対座標・GPU プリミティブ直接制御）の二層 WIT インターフェースである。上位層は Element Layer に element を作成し、スタイルを設定し、ツリーを組み立てる。Hayate 内部でレイアウト計算（Taffy）とスタイル解決を行い、Raw Layer のコマンド列に変換して GPU に送る。

```
┌──────────────────────────────────────────────────┐
│  TypeScript  Python  Go  Zig  C  C++  Rust  ...  │  ← 任意の言語（WIT SDK 自動生成）
├──────────────────────────────────────────────────┤
│          Hayabusa（Signal 型 Rust フレームワーク）  │  ← Hayate 公式フレームワーク
├──────────────────────────────────────────────────┤
│                                                  │
│    H A Y A T E  —  Element Layer / Raw Layer     │  ← ここ（WIT インターフェース）
│                                                  │
├──────────────────────────────────────────────────┤
│      WebGPU    Vulkan    Metal    DX12            │  ← GPU バックエンド（wgpu が抽象化）
└──────────────────────────────────────────────────┘
```

DOM 互換は設計目標に含まない。

## ステータス

🚧 **Step 1 — 実装中**（Canvas Mode: WebGPU + Vello でブラウザ canvas に GPU 描画）

## 技術スタック

| 役割 | 技術 |
|---|---|
| コア実装言語 | Rust |
| 公開インターフェース | WIT（WebAssembly Interface Types）+ wit-bindgen |
| GPU バックエンド | [wgpu](https://wgpu.rs)（WebGPU / Vulkan / Metal / DX12） |
| 2D レンダリング | [Vello](https://github.com/linebender/vello)（GPU compute shader、vendored） |
| レイアウト | [Taffy](https://github.com/DioxusLabs/taffy)（Flexbox / Grid / Block、vendored） |
| テキスト | [parley](https://github.com/linebender/parley) + [fontique](https://github.com/linebender/fontique) + [skrifa](https://github.com/linebender/skrifa)（vendored） |
| NodeId 管理 | [slotmap](https://github.com/orlp/slotmap)（generational arena） |
| WASM ビルド | wasm-pack |

## クレート構成

```
crates/
  core/              ← hayate-core（Scene Graph・レイアウト・レンダリングパイプライン）
  adapters/
    web/             ← hayate-adapter-web（WebGPU Canvas Mode + HTML Mode 自動切替）
  vendor/            ← ベンダリング依存（vello / taffy / parley 等）
```

## Web 動作モード

`hayate-adapter-web` はランタイムで自動検出し、最適なモードを選択する。

| モード | 条件 | 描画 | IME |
|--------|------|------|-----|
| Canvas Mode | WebGPU + EditContext API が両方利用可能（Chromium 系） | Vello GPU 描画 | EditContext API |
| HTML Mode | いずれかが利用不可 | element → HTML マッピング、native CSS | ブラウザ native |

## ドキュメント

- [設計仕様書](docs/hayate-spec.md)
- [ドメイン用語集](CONTEXT.md)
- [アーキテクチャ決定記録](docs/adr/)

## ライセンス

MIT — 商用利用・プロプライエタリフレームワークによる上乗せを制限しません。

依存ライブラリのライセンス（Vello 等 Apache-2.0 を含む）は `LICENSES/` ディレクトリで管理します。
