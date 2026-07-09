# Device Log は POST バッチで Dev Server に送る（WS 相乗りせず・Demo Endpoint には送らず・additive-only）

status: accepted

Date: 2026-07-09

## Context

Native Host のログは `console.*` → `__hayateLog` → logcat の一本道で、見るには USB/adb 接続が必須だった。
Torimi の売りは「バンドルをネットワークで流し込むだけ」なのに、診断だけケーブルに縛られるのは体験として破綻している。
そこで、端末上のログを開発機の Dev Server へネットワーク経由で届ける **Device Log** を導入する。

## Decision

- **捕捉範囲は 2 系統**：`js`（`console.*`・JS ランタイムエラー）と `host`（bundle 取得失敗・protocol version
  不一致・native エラー）。リモートログが最も要る瞬間は「画面が真っ黒で `console.*` がそもそも走らない」障害であり、
  `console.*` だけの仕組みは一番困る時に沈黙するため。この帰結として**送信主体はネイティブホスト**（JS 側送信機では
  host 系統を原理的に報告できない）。logcat 出力は併存させる。
- **wire は `POST /log/<deviceId>` のバッチ送信**。既存 `/reload` WS の上りに相乗りする案は却下 —
  dev-server の WS は意図的に「書くだけ」の最小実装で、上りを読むにはクライアント frame のマスク解除・分割処理を
  Node 側に手書き追加することになり、reload 用の 20 行がプロトコル実装に育つ。POST はバッチと自然に噛み合い、
  既存 `createServer` に 1 ルート足すだけで済む。契約（ルート・`LogBatch`/`LogEntry` 型）は
  `@torimi/dev-server-contract` に両側対等 import で置く。
- **Device ID はホスト発行・インストール単位のランダム不透明 ID**（ハードウェア由来 ID でもサーバ割当でもない）。
  表示用の Device Label（端末モデル名）はペイロードが毎回運び、ID に意味を焼き込まない。
- **送信ポリシー**：送るのは bundle 取得元が Dev Server のときだけ（**Demo Endpoint には決して送らない** —
  公開点に `/log` を持たせない）。通常は 2 秒間隔でバッファをまとめて POST（空なら無通信）、
  `error` と host イベントは間隔を待たず即時フラッシュ（クラッシュはプロセスが死ぬ前に外へ出せるかが勝負）。
  失敗時は上限付きリングバッファ（1000 件・古い方から破棄）で保持し次の間隔で再送、バックオフなし。
  端末ごと単調増加の `seq` を載せ、再送重複はサーバが `(deviceId, seq)` で捨てる（at-least-once）。
- **ホスト内の分業は ADR-0002 の流儀**：リングバッファ・`seq` 採番・フラッシュ判定・バッチ組み立ては Rust の
  transport 非依存純粋シーム、POST 実行は Kotlin（OkHttp）の注入ポート。将来の iOS（NSURLSession）で
  ロジックを二重実装しないため。
- **dev-server 側の分業**：wire 層（parse・検証・dedup、受理 204／壊れた JSON 400／1MB 超 413）は dev-server 本体、
  ターミナル表示＋ファイル追記はパッケージ内 sink ヘルパーとして提供し CLI（`torimi dev`）が配線する
  （startup-banner と同型）。dedup 状態（端末ごと最終 seq）はメモリのみ — dev-server はステートレスを崩さない。
  ファイルはプロジェクト cwd 配下 `.torimi/logs/<deviceId>/<YYYY-MM-DD>.torimi.log` に素のテキスト行で追記
  （日付振り分けはサーバ受信時のローカル日付、行内時刻は端末側 ts）。ローテーション・自動削除はしない。
- **互換性はバージョントークンなしの additive-only**。ホストは Play 配布の焼き込み済み、dev-server は npm 配布で、
  両端は独立に更新される。しかし log wire の不整合は最悪でも「ログが欠ける」だけでアプリ実行に無影響なので、
  Protocol Version 型の突き合わせ＋明示エラーはむしろ本末転倒（古いホストがログ体験ごと拒否される）。
  受け側は未知フィールドを黙って無視、送り側はフィールドの意味変更・削除・改名を禁止（変更は新フィールド追加で）。

## Considered Options

- **`/reload` WS の上りにログを流す**：接続＝端末生存の可視化が得られるが、Node 側 WS がプロトコル実装に育つ。
  生存の可視化はログが定期的に届くこと自体で代替できる。却下。
- **JS（prelude）側に送信機を置く**：host 系統のログ（JS 起動前に死ぬケース）を原理的に報告できない。
  native の JS に `fetch` が無い問題もある。却下。
- **log wire に Protocol Version 型の整合検査**：不整合の被害が描画壊滅（App Bundle wire）とログ欠落（log wire）
  では非対称で、後者に明示エラーは過剰。ベストエフォート受理が正しい失敗様態。却下。
