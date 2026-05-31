# Web IME は EditContext API 専用とし、非対応ブラウザは HTML Mode で対応する

> ⚠️ **部分的に上書き済み**: HTML Mode の描画アプローチ（Taffy → absolutely-positioned div）は
> **ADR-0029** によって廃止された。HTML Mode の現行アプローチはブラウザ CSS レイアウト方式。
> IME・モード選択・Canvas Mode に関する決定は本 ADR が引き続き有効。

## Context

Hayate の Web 実装（`hayate-adapter-web`）において IME 入力とレンダリングモードをどう扱うかを決定する必要がある。

当初は不可視 `<textarea>` + `compositionEvent` を Platform Adapter の内部実装として使用する方針だった（技術的負債として認識済み）。

## Decision

- **Canvas Mode** の条件: WebGPU（`navigator.gpu`）と EditContext API の両方が利用可能な場合。Vello + wgpu で GPU 描画し、IME に EditContext API を使用する
- **HTML Mode** の条件: WebGPU または EditContext API のいずれかが利用できない場合。element tree を HTML にマッピングし、IME はブラウザ native に委ねる
- モード選択はランタイム自動検出。アプリ側はモードを意識しない
- Canvas Mode を永続的にサポートしないブラウザへの対応は行わない

## Considered Options

- **不可視 textarea（却下）**: 全ブラウザで動くが、DOM に触れるという設計原則違反が残る。EditContext が普及する前の一時的な対応に過ぎず、将来の置き換えコストが生じる
- **EditContext + textarea フォールバック（却下）**: 二つの IME パスを維持するコストが高く、テストマトリクスが倍になる
- **EditContext 専用 + HTML Mode（採用）**: IME パスが一本に絞られる。HTML Mode は独立した描画パスとして品質を確保できる

## Consequences

- Canvas Mode は Chromium 限定になる。Firefox / Safari が EditContext を実装しない限り Canvas Mode では動作しない
- HTML Mode は SolidJS 相当の描画性能を持つ独立したパスとして設計・テストされる
- JS-WASM ブリッジのオーバーヘッドはナノ秒オーダーであり、HTML Mode の性能ボトルネックにはならない
- 開発ロードマップ: Step 1 で Canvas Mode（Vello 描画）、Step 2 で HTML Mode を多言語 SDK とともに先行リリースして WIT インターフェース全体の検証を行う
