# バンドル取得・reload 購読のネットワーク transport を OS プラットフォームの HTTP スタックへ委譲する

> **用語更新（ADR-0004・2026-07-07）**: 本 ADR が言及する "Miharashi" は **Torimi（鳥見）** に改名された（ディレクトリ・`@torimi/*` パッケージスコープ・wire グローバル等の全面リネーム）。本文は決定当時の記録として原文のまま。

status: accepted

Date: 2026-07-06

## Context

Miharashi を Google Play に公開するにあたり、テスター・審査者がいつでも動かせるデモ App Bundle を
公衆インターネット（Cloudflare）上の常時配信点から取得する必要が生じた。公衆配信は HTTPS が前提になる
（配信先の Cloudflare は HTTP を 301 で HTTPS へ誘導する上、eval される実行可能 JS の平文配信は経路上の
MITM がそのままテスター端末上の任意コード実行になるため、セキュリティ上も Play ポリシー上も擁護できない）。

一方、現行 Android ホストの transport は**依存追加なし**を優先した手書き実装だった：

- バンドル取得＝素の `TcpStream` 上の最小 HTTP/1.1（`bundle_source.rs`）。TLS を話せず、リダイレクトも追わない。
- reload 購読＝RFC6455 ハンドシェイクを手書きした素の TCP WS クライアント（`reload_socket.rs`）。

「依存追加なし」は LAN 内 dev（開発機の dev-server に http で繋ぐ）だけが要件だった当時の判断であり、
公開配信という新要件はその前提を破る。Rust 側に TLS（rustls 等）を入れる案は、証明書検証の platform 統合
（特に将来の iOS）を自前で抱え込むことになり、iOS 対応の負債になり得る。

## Decision

- **ネットワーク transport（バンドル fetch の HTTP(S)・reload 購読の WS(S)）は OS プラットフォームの
  ネットワークスタックに委譲する。** Android は Kotlin 側（OkHttp 系）が fetch / WS を担い、結果（JS ソース
  文字列・reload シグナル）を既存の注入シーム（`boot_runtime` の `fetch`、`subscribe_reload` のポート）経由で
  Rust に渡す。将来の iOS は NSURLSession が同じ席に座る。**Rust ホストに TLS 依存は入れない。**
- これは Expo Go と同型（TLS を自前実装せず、Android=OkHttp / iOS=NSURLSession に委譲。dev は http、
  公開配信は https）。また既存原則「host bootstrap（surface 取得・native glue）はネイティブ側が持つ」
  （ADR-0112 / docs/adr/0004 の系譜）に transport も含める、という整理である。
- **既存の LAN dev 経路も同じ委譲実装に統一する**（素の TCP 実装と平文専用経路を並存させない）。LAN dev の
  平文 http は networkSecurityConfig で cleartext をローカル用途に明示許可して維持する。
- Rust 側に残るのは transport 非依存の純粋シーム（boot の順序付け・protocol version 突き合わせ・reload の
  意味づけと backoff orchestration）のみ。手書き HTTP/1.1・RFC6455 実装（`build_bundle_request` /
  `parse_bundle_response` / `reload_socket.rs` のハンドシェイク組み立て）は委譲完了をもって撤去する。

## Considered Options

- **rustls + rustls-platform-verifier を Rust ホストに追加**：fetch が Rust に閉じる利点はあるが、TLS スタックと
  OS 信頼ストア統合の保守を自前で抱え、iOS 対応時に同じ問題をもう一度解くことになる。却下。
- **平文 http のまま公開配信（カスタムドメイン + Always Use HTTPS off）**：アプリ変更ゼロだが、公衆網越しの
  実行可能コード平文配信は MITM→任意コード実行の構図で、テスターに対して擁護できない。iOS の ATS は https を
  強制するため、将来の iOS 対応でどのみち破綻する。却下。

## Consequences

- HTTPS/WSS はプラットフォームから無償で得られ、公開デモ配信点（Cloudflare）にそのまま繋がる。
- ホストの「薄いシェル」性が強まる：ネットワーク I/O はネイティブ層、意味づけ（boot 順序・version 突き合わせ・
  reload）は Rust の純粋シーム、という分業が transport にも一貫する。
- Kotlin↔Rust の受け渡しシームが transport の wire 契約になる（従来の「HTTP バイト列の解釈」から
  「取得済み文字列/シグナルの受け渡し」へ契約の高度が上がる）。契約テストは Rust 純粋シーム側で維持する。
- Android で cleartext を許可する範囲は networkSecurityConfig で LAN dev 用途に限定する（既定は https）。
  - **訂正（2026-07-07）**: networkSecurityConfig は IP レンジ（192.168.0.0/16 等）を表現できず、
    「LAN dev 用途に限定」は設定で実装不可能だった。本 ADR の本旨（Expo Go と同型：dev は http、
    公開配信は https）に従い、アプリは release を含め cleartext を全面許可する（Expo Go の Play
    配布版と同じ）。公開 Demo Endpoint 側が HTTPS であることは変わらない。
