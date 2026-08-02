# Torimi を Tauri 製の単一 WebView シェルにし、Live Preview の信頼は静的 ACL ではなく実行時に承認した Paired Origin で表す

status: accepted

Date: 2026-08-02

## Context

Torimi は Tsubame の App Bundle を実機で動かす dev-client として作られた（ADR-0001）。しかし実際の
開発では、Hayate ネイティブ経路の検証と、開発機（LAN の dev server またはクラウドサンドボックス）で
動いている普通の Web アプリの実機確認とを、**同じ 1 台の端末アプリで**行き来したい。後者は App Bundle
でもフレームワーク指定でもなく、URL が指す物をそのまま開くだけの経路である。

そこで Torimi の外延を「Tsubame アプリのプレビュー」から「開発機で動いているものの実機プレビュー」へ
広げ、Preview Mode を Bundle Preview と Live Preview の 2 つに分ける。Shell の UI（接続先入力・QR・
履歴・ログ・承認）を Web 技術で書くため、シェルには Tauri 2 を採る。

このとき決めなければならないのは、**Torimi が所有しないコードを載せる面（Live Preview）に、
ネイティブ機能をどう出すか**である。Tauri の capability は remote origin に対して
`remote.urls`（URLPattern）で静的に許可を書くモデルだが、Torimi が接続する先は**実行時にしか
分からない**。dev server は `http://192.168.x.y:5173` のこともあれば、クラウドサンドボックスの
公開 HTTPS ホストのこともある。ビルド時に列挙することは原理的にできない。

## Decision

- **Torimi Shell と Live Preview は同一の WebView を切り替えて使う**（単一 WebView 構成）。Shell は
  ローカル origin の文書、プレビュー対象は remote origin の文書で、同時には存在しない。
  プレビュー用に隔離した第二の WebView は採らない — Android では 1 window 内の複数 webview が
  サポート外であり、複数 window は別 Activity かつ API 32+ を要求して `minSdk` 24 を割るため。
- **静的な `remote.urls` は空にする。** ワイルドカードによる許可（`http://*`、私有 IP レンジ、
  既知サンドボックスドメインの列挙）はいずれも採らない。前二者は「LAN にいること」を承認と
  取り違える設計であり、最後は接続先を変えるたびにアプリの再ビルドと Play 更新を要求する。
- **信頼は Paired Origin で表す。** 人が Torimi Shell 上で接続先を承認した時点で、その
  **厳密な 1 origin だけ**を対象とする capability を実行時に追加する（`dynamic-acl` feature の
  `Manager::add_capability`）。承認は capability ごとに個別で、別の origin へ移れば失効する。
- **公開するのは Torimi Command だけ。** 使用フレームワークの公式プラグインが持つ command を
  remote 側の capability に紐付けることはしない。Live Preview のページから見えるのは、Torimi が
  明示的に載せた操作の面に限る。
- **実行時ゲートを併置する。** 各 Torimi Command は自身の中で、呼び出し元 WebView の現在 URL
  （`Webview::url()`）を現在の Paired Origin と照合する。capability の取り消し API が無い場合でも、
  承認の失効はこのゲートで確実に効かせる。
- **Tauri は 2.11.1 以上にピン留めする。** `is_local_url()` の origin 混同（GHSA-7gmj-67g7-phm9、
  CVSS 6.1）は Android を影響プラットフォームに含み、remote ページがローカル専用 command を
  呼べる欠陥だった。本 ADR の構成はまさにその面を使うため、修正版を下限とすることは任意ではない。
- **特定フレームワークとの互換は約束しない。** Live Preview が保証するのは「dev server が配信する
  Web アプリが実機の WebView で動くこと」と「Torimi が列挙した capability に Torimi Command から
  届くこと」の 2 つだけである。既存のネイティブプラグイン呼び出しが自動的に満たされることは
  約束しない — それはホスト側の再ビルドを要求するものであり、事前ビルド済みホストという
  Torimi の前提と両立しない。

## Considered Options

- **プレビュー専用の非特権 WebView に隔離する。** 最も安全だが、Android では 1 window 複数 webview が
  desktop 専用であり、複数 window は別 Activity ＋ API 32+ で `minSdk` を割る。
- **iframe に隔離する。** 採用不能。Tauri は Android と Linux では iframe からの要求と window 本体
  からの要求を区別できないと明記しており、対象プラットフォームでまさに成立しない。
- **local origin proxy（dev server をローカルプロトコル経由で配ってローカル扱いにする）。** Tauri の
  通常 ACL がそのまま効く利点があるが、Torimi が所有しないコードに**ローカル origin の特権を与える**
  ことになり目的と逆向きである。加えて HMR / WebSocket / CORS の再現性が読めない。
- **静的に広く許可し実行時ゲートだけで守る。** 公式ドキュメントが「全 remote URL への API 許可は
  サポート外かつ強く非推奨（RCE・データ漏洩）」と名指しで否定している。

## Consequences

- **受容するリスク**: Torimi が所有しないコードを、Shell と同じ WebView に載せる。隔離の実効性は
  最終的に Tauri の Android 実装の正しさに依存し、実際にその面で欠陥（上記 advisory）が発生した
  実績がある。Torimi は開発者が**自分のアプリ**を実機検証するための道具であり、敵対的な第三者の
  アプリを読み込ませる用途は想定しない。この前提の上でリスクを受容する。
- Bundle Preview の Protocol Version 整合（ADR-0001 系）は Live Preview には存在しない。Live Preview
  に流し込まれるのは Torimi の契約を何も知らない任意の Web アプリであり、握手する相手がいない。
- **Live Preview を Device Log の対象に含めるかは本 ADR では決めない。** 実機 WebView では
  ブラウザ devtools が手近にないため、Web Host を除外した理由（ADR-0005）はここには当てはまらない。
  必要性が実運用で確認できた時点で別途決める。
