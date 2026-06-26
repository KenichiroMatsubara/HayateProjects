# Miharashi は独立 context：Hayate ネイティブ runtime を再利用し、FW 非依存ホストへバンドルが FW＋renderer-canvas を持ち込む

status: accepted

Date: 2026-06-25

## Context

Tsubame アプリを端末上でプレビューする dev-client（Expo Go 相当）を作る。ADR-0112 で
Android の `hayate-adapter-android` cdylib に Hermes を埋め込み、`RawHayate` JSI ブリッジ越しに
Tsubame JS（`tsubame-solid` + `@tsubame/renderer-canvas` + Todo）をネイティブ Hayate 上で実行する
経路は既に実体化している。ただしバンドル源は**ビルド時固定**で、`app_tsubame.rs::load_bundle()` が
APK assets の `tsubame.js` を読むだけ。dev-client の固有価値は、このバンドル源を**実行時・ネットワーク**化し、
ホストを再ビルドせずアプリだけ差し替えて reload することにある。

この製品をどこに、どんな境界で置くかに二つの分岐があった。

1. **配置**：ネイティブシェルが Hayate の Android platform adapter と密に絡む（cdylib・Hermes・JSI）ため
   Hayate 配下に置くか、独立 context にするか。
2. **FW 結合**：ホストにフレームワーク（solid/react/vue）や `@tsubame/renderer-canvas` を焼き込むか、
   ホストは最小の host bootstrap だけ持ち、バンドルが FW と renderer-canvas を持ち込むか。

Hayabusa（signal ランタイムを Rust が単独所有し WASM/ネイティブにコンパイル）は対象外とする — iOS が
「ダウンロードしたネイティブ/JIT コードの実行」を禁じ、インタープリタ実行の JS のみ許す以上、dev-client で
配って実行する形が iOS で原理的に成立しない。Tsubame JS（Hermes インタープリタ実行）は通る。

## Decision

- **Miharashi を新規トップレベル context とする**（`Miharashi/`）。鳥（フレームワーク）でも風（Hayate＝基盤）でも
  ない第三カテゴリの**道具**。Hayate のネイティブ runtime（ADR-0112 の Hermes 埋め込み＋`RawHayate` ブリッジ
  cdylib 能力）を**再利用**し、複製しない。Tsubame を消費する App（合成ルート）であり、Tsubame の renderer
  パッケージには属さない（ADR-0004）。
- **ホストは FW 非依存**。ホスト（Web／native とも）が提供するのは host bootstrap（JS エンジン・ネイティブ／web
  Hayate・`RawHayate`・frame clock）だけ。**フレームワークも `@tsubame/renderer-canvas` も持たない。**
- **App Bundle が FW＋renderer-canvas＋app を持ち込む**。solid/react/vue は*別のバンドル*にすぎず、ホストは中身の
  FW を解さず `RawHayate` を満たす JS として実行するだけ。これは ADR-0112 のバンドル構成（esbuild で
  renderer-canvas＋adapter＋app を 1 つにまとめる）と一致する。
- **protocol version で整合を取る**。バンドル内 renderer-canvas の wire 定数バージョンと、ホストに焼き込まれた
  ネイティブ decoder のバージョンをバンドルに埋め、起動時に突き合わせ、不一致は明示エラー（Expo の SDK version
  整合と同型）。
- **Web 先行・Android で完成**。Web トレーサーは `hayate-adapter-web` の auto モードで native と同じ Canvas／
  `apply_mutations` シームを検証する（auto は `RawHayate` より下の内部選択で Miharashi の構造に影響しない）。
  Android は `load_bundle()` をネットワーク fetch に差し替え、WS reload・version チェック・URL UI を足す。

## Considered Options

- **ネイティブシェルを Hayate 配下に置く**：境界は減るが、Miharashi という製品（dev server・bundle・pairing・
  reload・version）が複数 context に散り、語彙の所有が曖昧になる。却下。
- **ホストに FW／renderer-canvas を焼き込む**：FW ごと・バージョンごとにホスト再ビルドが要り、「Viewer 一本で
  全 JS FW が動く」が崩れる。dev-client の前提に反するため却下。

## Consequences

- 「Viewer 一本で solid/react/vue を、ホスト再ビルドなしに動かす」が構造で保証される。FW 追加＝バンドル追加。
- ホストとバンドルの wire 整合は protocol version に集約され、ズレは謎クラッシュでなく明示エラーになる。
- HMR を将来入れても、HMR ランタイムはバンドル側（FW 固有 fast-refresh）に置き、ホストは WS を JS に中継するだけ
  なので、full reload／HMR でホストのネイティブ契約は不変。
- この remote 環境には Android SDK/NDK/実機が無いため、Android stage は host-readable contract テスト＋ローカル
  実機で検証する（ADR-0112 と同じ制約）。
- Hayabusa は dev-client 対象から恒久的に外れる（iOS 制約）。

## 関係

- ADR-0112：Android が埋め込み Hermes 越しに Tsubame JS を実行する経路。Miharashi はこのバンドル源を
  asset → ネットワークへ一般化して再利用する。
- ADR-0004（Tsubame）：host bootstrap は Tsubame の renderer パッケージに置かない。Miharashi は App 側として
  host bootstrap を所有する。
- ADR-0117（Hayate）：App Host の boot シーム（`tick`／`request_redraw`／`DeliverySink`）。Miharashi の reload は
  この上に乗る。
- ADR-0121（Hayate）：WebView+wasm ニア・ネイティブ経路。下記訂正の根拠。

## 訂正（amendment・2026-06-26）

Context／Consequences の **「Hayabusa は dev-client 対象から恒久的に外れる」「dev-client で配って実行する形が
iOS で原理的に成立しない」は過剰主張**だった。**WASM-in-WebView を見落としている** — iOS が禁じるのはアプリ
自身のプロセスで走る、落としてきた native／JIT コードであって、**WKWebView 内の wasm は許される**（Capacitor が
js↔native ブリッジで審査を通している実績がそれを裏づける）。

したがって Hayabusa の iOS 不可は **「ダウンロード native ターゲット」かつ「本 ADR が採る非 webview ルート
（Hermes/JSI 直結）」に限った話**であり、capability や言語の本質ではない。**ADR-0121 の WebView+wasm ルート
（webview 上の canvas に wasm を wgpu 描画／IME はネイティブ API／native は wasm→js→native ブリッジ）を使えば、
原理的には iOS でも Hayabusa を載せられる**。

ただし **Hayabusa 自体がまだ未完成のため、これは未来の話**である。本 ADR の現行ルート（Tsubame 専用・非
webview）の決定はそのまま有効で、変更しない。訂正するのは「恒久／原理的不可」という結論の一点のみ
（除外は現行ルート限定であって恒久ではない）。
