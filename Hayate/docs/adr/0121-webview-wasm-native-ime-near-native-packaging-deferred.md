# WebView ＋ wasm ＋ ネイティブ IME によるニア・ネイティブ パッケージング経路（記録のみ・未着手）

> **用語更新（ADR-0004・2026-07-07）**: 本 ADR が言及する "Miharashi" は **Torimi（鳥見）** に改名された（ディレクトリ・`@torimi/*` パッケージスコープ・wire グローバル等の全面リネーム）。本文は決定当時の記録として原文のまま。

status: noted（記録のみ・未着手）

Date: 2026-06-26

## Context

Hayabusa／Tsubame の wasm アプリを、**ブラウザではなくネイティブアプリ（WebView 同梱）として
iOS／Google のアプリ審査基準を満たしつつ、ネイティブとほぼ一致した動作で配布・検証する**経路が
原理的に成立しうると判明した。Hayabusa（Rust 単独所有のリアクティブランタイム・ADR-0045）が
「できないわけではない」ことの裏付けでもある。今は着手しないが、後で参照できるよう記録だけ残す。

## 記録する経路（アイデア）

1. **描画**: WebView 上の Canvas に wgpu（WebGPU）で全 UI を GPU 描画し、アプリロジックは wasm
   で動かす（Canvas Mode 同型を webview 内で再現）。
2. **IME**: その Canvas の text-input を、**ブラウザの IME ではなくネイティブアプリ用の IME API**
   で駆動する。Hayate が既に持つネイティブ IME 経路（`ImeBridge`／`EditIntent`・ADR-0017/0069）を
   webview 上の描画面に接続し、ブラウザ EditContext（ADR-0016）には依存しない。
3. **native API**: `wasm → js → native` 経路でネイティブ機能（capability 等）を利用する。
   Capacitor が js↔native の API ブリッジを実証済みなので方式自体の妥当性はある。**残る技術課題は
   wasm↔js 間でのネイティブデータの受け渡し**（境界をまたぐ encode/decode）。
4. **効果**: WebView を内包するネイティブアプリとして（Capacitor 系アプリ同様）ストア審査を通し
   ながら、ネイティブとほぼ一致した動作を TestFlight で検証できる。

## なぜ今やらないか

- 配布・パッケージング層の大きな新経路で、IME・webview・wgpu・wasm・js-native ブリッジを束ねる
  統合コストが大きい。現フェーズ（capability scaffold・ADR-0119／wave-2 契約・ADR-0120）の射程外。
- wasm↔js のネイティブデータ受け渡しが未検証の核心リスクで、spike が要る。

## 関係

- **ADR-0120（wave-2 ストリーム capability 契約）**: capability を wire/JS へ公開する将来軸の
  **発火トリガ候補**。本経路を採るなら、JS（webview 内 wasm）から native capability に届く必要が
  生じ、`proto/spec` の mobile セクション投影（in-process 先行・wire 延期の解除）が現実化する。
- **ADR-0016（web IME = EditContext のみ）**: 本経路はブラウザ EditContext ではなくネイティブ IME を
  使う点で、その前提の例外軸にあたる。
- **ADR-0045（Hayabusa = in-process Rust）／ADR-0035（Hayabusa wasm）**: Hayabusa wasm を
  ネイティブ同梱で配布する具体経路の記録。
- **Miharashi ADR-0001**: 同 ADR の「Hayabusa は dev-client 対象から恒久的に外れる／iOS で原理的に
  成立しない」という結論を**訂正する根拠**。本経路（WASM-in-WebView）が成立するため、Hayabusa の iOS
  不可は非 webview ルート限定であって恒久ではない（Hayabusa 未完成ゆえ未来の話ではある）。
