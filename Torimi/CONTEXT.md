# Torimi Glossary

**Torimi（鳥見）** は、Tsubame アプリを端末上でプレビューする**フレームワーク非依存の dev-client**である。語の二重性は世界観に沿う — バードウォッチング用語の「鳥見」（探鳥＝**鳥（フレームワーク）の動きを見る道具**）と、江戸幕府の役職「鳥見役」（鷹場を見張り整える番人＝Hayate・Tsubame・Hayabusa が飛ぶ場を見守る立場）の二重の由来を持つ。「鳥*が*見る（鳥瞰）」ではなく「鳥*を*見る」側の語であることが要点（ADR-0004）。鳥（フレームワーク）でも風（基盤）でもない第三カテゴリの**道具**。

> 語彙の正本。各語が**何であるか**を定義する。実装の仕組み・決定は各 ADR に置き、ここには書かない。

## Core Terms

**Torimi（鳥見）**:
事前ビルド済みのネイティブホストに、Tsubame Adapter の JS バンドルをネットワーク経由で流し込んで実行・プレビューする dev-client App。フレームワーク非依存で、solid / react / vue のいずれのアプリも*別のバンドル*として同一ホストで動かす。Expo Go と同じ立ち位置（ホストは再ビルドせず、バンドルだけ差し替える）。
_Avoid_: Tsubame Viewer（Tsubame context の一部だと誤読される）, フレームワーク, ランタイム, example ギャラリー

**Host（Torimi ホスト／Native Host）**:
端末側に常駐する事前ビルド済みシェル。JS エンジン（Hermes）・ネイティブ Hayate・`RawHayate` ブリッジ・frame clock（host bootstrap）だけを提供し、**フレームワークも `@tsubame/renderer-canvas` も持たない**。ADR-0112 の `hayate-adapter-android` cdylib 能力を*再利用*する（複製しない）。Web Host と対をなすホスト種別の一方で、無印の「ホスト」はこちら（ネイティブ）を指す。消費する App Bundle は **Hermes 用に降格済み**でなければならない点が Web Host との差。ホスト種別の軸は OS（Android / iOS）とは別軸 — 現行実装が Android のみなのは実装状況であって、Native Host の定義は OS を固定しない。
_Avoid_: フレームワークをホストに焼き込む設計, renderer-canvas をホスト側に置く設計, Android Host（OS 軸との混同 — Native Host の現行実装が Android というだけ）

**Web Host（Torimi Web ホスト）**:
ブラウザ上で App Bundle を実行するもう一方のホスト種別（`@torimi/host-web`）。Dev Server からバンドルを fetch → eval し、protocol handshake の後、host bootstrap を mount に渡す。Native Host と同じ Dev Server 契約・同じ mount 契約（`__torimiMount`）を見るが、消費するバンドルは **es2020 のまま**（Hermes 降格不要）で、Native Host 用バンドルとは**別成果物**。E2E テストと開発機上の即時確認が主用途。
_Avoid_: Hayate の HTML Mode との混同, DOM Renderer 経路との混同（Web Host は Hayate Renderer 経路のホスト）, Native Host 用の降格済みバンドルを流用する運用

**App Bundle（アプリバンドル）**:
Torimi に流し込まれる JS。アプリコード ＋ Tsubame Adapter（solid / react / vue ランタイム）＋ `@tsubame/renderer-canvas` を 1 つにまとめたもの。ホストは中身のフレームワークを解さず、`RawHayate` を満たす JS として実行するだけ。Hayabusa（WASM／ネイティブ）は**現行（Hermes/JSI 直結・非 webview）ルートの**バンドル対象外（現ルートでは iOS で配って実行できない）。ただしこれは**恒久ではない** — Hayabusa 自体がまだ未完成のため未来の話だが、WebView+wasm ルート（ADR-0121：webview 上の canvas に wasm を wgpu 描画・IME はネイティブ API・native は wasm→js→native ブリッジ）を使えば、**原理的には iOS でも Hayabusa を載せられる**。
_Avoid_: フレームワークをバンドルから除く設計, 現行ルートで Hayabusa をバンドルする設計, Hayabusa の iOS 不可を恒久／原理的と読む理解（現行ルート限定であり webview+wasm で将来可能・ADR-0121）, `.hbc` 固定（配信形式は別決定）

**Dev Server**:
開発機上で動き、ファイル変更を監視して App Bundle を生成し、HTTP で配信、WS で reload／更新シグナルを送るツール。HMR 時は差分モジュールを WS で送り、HMR ランタイムは**バンドル側**が持ち込む（ホストは WS を JS に中継するだけで HMR を解さない）。
_Avoid_: ホスト側に FW 固有 fast-refresh を持たせる設計

**Torimi CLI（`torimi`）**:
外部アプリの開発ループを 1 コマンドに束ねるオーケストレータ。npm の**無スコープパッケージ `torimi`**（bin も `torimi`、設定は `torimi.config.*`）で、Expo における `expo` コマンドと同じ立ち位置。アプリが宣言したビルドコマンドを watch 付きで回し、ターゲットに応じた降格（Hermes 用 lowering はターゲット固有＝CLI の責務）を施し、`@torimi/dev-server` による配信・reload・QR まで面倒を見る。**FW もビルドツールも解さない**（ビルドコマンドは不透明な設定値）— FW 知識は Tsubame 側の preset に、配信は下層の Dev Server に、それぞれ隔離されたままにする。
_Avoid_: CLI がバンドラ・FW プラグインを内蔵する Expo/metro 型の理解, `@torimi/dev-server` との同一視（CLI はその上のオーケストレーション層）, Tsubame 所属のツールとする理解（FW 非依存であり所属は Torimi）

**Demo Endpoint（デモ配信点）**:
テスター・審査者向けに、ビルド済みデモ App Bundle と Demo Manifest を常時 HTTPS 配信する公開点。watch もビルドも行わず、reload シグナルも発しない（WS は受けて保持するだけ）点で Dev Server とは**別物**。Play 配布ホストの既定接続先であり、開発機の稼働と無関係に「いつでも」応答する。
_Avoid_: Dev Server と混同（Dev Server は開発機上で watch して動くツール）, main 追従の自動更新（配信物はリリースと lockstep・ADR-0003）

**Demo Manifest（デモマニフェスト）**:
Demo Endpoint が配信するデモ一覧（各エントリ＝表示名とバンドル URL）。ホストはこれを取得してデモ選択メニューを構成し、初回起動は先頭デモを自動ロードする。デモの追加・改名はマニフェスト更新であり、ホストのアプリ更新（Play 審査）を要しない。
_Avoid_: デモ一覧のホストへのハードコード, フレームワーク知識のホスト側持ち込み（エントリは不透明なバンドル URL）

**Protocol Version**:
App Bundle 内の `@tsubame/renderer-canvas` が内包する wire 定数のバージョンと、ホストに焼き込まれたネイティブ decoder のバージョンの整合トークン。バンドルに埋め、Torimi 起動時に突き合わせ、不一致は明示エラーにする（Expo Go の "SDK version" 整合と同型）。
_Avoid_: 無検査での流し込み, decoder の暗黙後方互換前提

**Reload**:
バンドル変更を端末に反映する仕組み。暫定は **full reload**（バンドル全体を取り直し JS ランタイムを再構築、state は飛ぶ）で全 FW に一様に効く。目標は **HMR**（差分モジュール差し替え・state 維持）だが、FW 固有 fast-refresh はバンドル側に置き、ホストのネイティブ契約は full reload／HMR で不変。
_Avoid_: ホスト側 HMR ランタイム, Hayabusa の "Hot Reload"（別 context の別語）

## Related Contexts

**Hayate**:
Torimi のネイティブ実行基盤。Torimi ホストは Hayate のネイティブ runtime（ADR-0112 の Hermes 埋め込み＋ `RawHayate` ブリッジ cdylib 能力）に依存する。

**Tsubame**:
Torimi が消費する対象。App Bundle は Tsubame Adapter ＋ `@tsubame/renderer-canvas` を内包する。Torimi は Tsubame の renderer パッケージには属さない App（合成ルート）である（ADR-0004）。
