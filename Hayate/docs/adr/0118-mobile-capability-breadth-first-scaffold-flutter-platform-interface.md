# モバイル capability を Flutter `platform_interface` モデルで breadth-first に scaffold し、各 capability を「Core trait ＋ android/ios 両 leaf stub ＋ mobile facade ＋ typed Unimplemented エラー」で先に型として存在させる

status: accepted

Date: 2026-06-25

## Context

ADR-0117 が adapter 層を Core / Family Adapter / leaf の三層に再編し、`crates/platform/README.md`
が grouping doctrine（契約は Core / **昇格は 2 実装から** / **trait を先置きしない** / 借りるのは
taxonomy のみで機構は借りない）を正本化した。この doctrine 下で実際に共通 API の表面に載っている
capability は **audio 一つだけ**で、clipboard・haptics・notification 等は未着手だった。

方針として「モバイル共通 API の**概形を先に全部作る** — クオリティが低くても、動かなくても、
**呼べば必ず “ちゃんとエラー” を返す**状態を全 capability で実現する」を採りたい。これは
breadth-first scaffold であり、doctrine の「昇格は 2 実装から / trait を先置きしない」と正面から
ぶつかる。何をモデルに据え、衝突をどう折り合わせ、エラーをどう表現し、どこまでを対象にするかを
決める必要がある。

## Decision

### モデル：Flutter federated plugin の `platform_interface` を主・RN core module をカタログ網羅チェック

- 構造モデルは **Flutter の federated plugin ＋ `*_platform_interface` パターン**。`platform_interface`
  は抽象契約を 1 箇所が所有し未実装は既定でエラーを **throw** する — これは ADR-0117 の「契約は Core /
  実装は leaf」とほぼ同型で、しかも「動かないにしてもちゃんとエラーを出す」状態そのもの。
- **RN（TurboModule）の core module 一覧はカタログ網羅性のクロスチェックにのみ使う。** 機構（bridge /
  channel のランタイム dispatch）は借りない（README 鉄則3を維持）。
- 借りるのは **taxonomy（どの機能がどの段か）と throw-by-default な契約の形**だけ。throw-by-default は
  エラー signaling の流儀であって機構ではないので鉄則3に反しない。

### ゲートの再フレーム：「2 実装」を「2 leaf stub ＋ Flutter 由来の契約形」と読み替える

- doctrine 鉄則2「昇格は 2 実装から」が守りたいのは「**1 platform 決め打ちで契約形を誤る**」リスク。
  本決定は各 capability を **android+ios の leaf stub を同時に**置き、契約形を Flutter `platform_interface`
  （複数 platform の variation を織り込んだ prior art）から取ることで、このリスクを回避する。
- よって breadth-first scaffold は doctrine を override せず、ゲートの**意図**を満たす形に収める。
  **明示的に受容するリスクは「実機実装が入ると契約の形が変わりうる」一点。** 鉄則1（契約は Core）と
  鉄則3（機構は借りない）は不変。
- 「概形を**完璧に**設計」は誇張なので「**網羅的・型付き・呼べば typed エラーを返す**」に下方修正する
  （契約の最終形は実装で確定する）。

### エラーモデル：`Result<T, CapabilityError>`、stub は `Err(Unimplemented)`、panic 禁止

- capability メソッドは原則 `Result<T, CapabilityError>` を返す。Flutter の throw を Rust の
  `Result::Err` へ写像する（catchable な等価物は panic ではなく `Err`）。
- **panic（`unimplemented!()` / `todo!()`）は使わない。** leaf は Kotlin/Swift への FFI 境界で、Rust
  panic は FFI 越えで abort/UB（既存 adapter も panic を避けている）。
- `CapabilityError`（**Core 所有**）の初期 variant は `Unimplemented{capability, platform}` /
  `Unsupported{capability, platform}` / `Platform{code, message}` の 3 つ。`PermissionDenied` 等は
  最初の権限ゲート付き capability を実機実装する時に足す（error variant にも「先置きしない」を適用）。
- **audio は infallible 据え置きの例外。** `AudioOutput`（open/submit/close）は shipped かつ realtime
  hot-path で返す意味が無いため `Result` 化しない。新規 scaffold capability のみ `Result` を返す。

### 対象境界：薄い native capability のみ（既所有・重量級は除外）

- 既に Core/Platform Front/Adapter が所有する領域（IME/viewport/raw 入力/surface/font/テーマ）は
  capability に含めない。**clipboard も含めない** — ADR-0014 が Platform Adapter の責務に clipboard を
  明記し、ADR-0097 が編集境界 `element::clipboard::Clipboard`（選択テキストのコピペ経路）として実装
  済み。同一 OS API（Pasteboard/ClipboardManager）への 2 重抽象を避ける。app からの programmatic
  clipboard が要る時は ADR-0097 の trait を拡張する（並行 trait を切らない）。
- 重量級サブシステム（camera / video_player / webview / google_maps / in_app_purchase）も含めない
  （薄い trait ではなく独立プロダクト）。

### 置き場：全 scaffold は Core trait ＋ `platform/mobile/` facade ＋ android/ios stub

- 今 scaffold するものは段の予測に関係なく **`platform/mobile/` facade（`MobileAudioOutput` と同型の
  `cfg(target_os)` 型 alias）＋ Core trait ＋ android/ios stub** に置く。
- 「最終的な段（common/family）」はカタログ上の**予測列**であって、`platform/common/` への昇格は
  web/desktop の実 stub が揃った時。`platform/common/` ・ `platform/desktop/` は枠のまま空に保つ
  （空 facade を先置きしない）。

### phasing：wave-1 = 一発応答 10 個 / wave-2 = ストリーム 4 個 / permissions = さらに後

- **wave-1（一発応答・fire-and-forget／正しい形で scaffold 可・計 9 個）**: haptics・notification(local)・
  share・file picker・url launcher・key-value storage・secure storage・biometric(local auth)・device info。
  （当初 clipboard を含めていたが、ADR-0097 の編集境界と重複するため上記境界で除外した。）
- **wave-2（ストリーム型／query ＋ 状態変化イベントの連続供給）**: battery・connectivity・geolocation・
  sensors。これらは event/stream 契約が要り、プロジェクト固有のイベント経路（`DeliverySink`/poll・
  ADR-0117）に乗せるか別契約にするかが独立した設計分岐。今 stub すると形を誤る確率が高いので wave-2 へ。
- **permissions はさらに後。** platform 乖離が最大（iOS=使用時に framework API ごと暗黙要求 /
  Android=`requestPermissions` 文字列）。doctrine 通り「権限ゲート付き capability が実機で 2 つ実装されて
  から」設計する。今は trait も `PermissionDenied` variant も置かない。

## Considered Options

- **doctrine を正面から override（scaffolding フェーズは鉄則2を停止）**: シンプルだが README 正本と
  矛盾が残り揺り戻しやすい。ゲートの意図を満たす再フレームで同じ breadth を得られるため却下。
- **コードを作らず紙のカタログだけ先に作る**: doctrine 完全準拠だが「stub でも作って呼べばエラーを返す」
  という目的を満たせない。却下。
- **stub を `unimplemented!()` で panic**: 最小手数だが FFI で abort、「ちゃんとエラー」ではない。却下。
- **RN を主モデルにする**: 機構が bridge 前提で鉄則3と相性が悪い。カタログ網羅チェックに留める。
- **15 個全部を wave-1 で scaffold**: streaming 契約を未設計のまま stub 化 → 形を誤る。wave 分割で回避。

## Consequences

- モバイル共通 API の**全表面が型として早期に可視化**され、呼べば `Err(Unimplemented)` を返す。実装の
  進捗は「`Unimplemented` を返す stub → 実装」の差し替えで進む。
- `CapabilityError` が Core に新設され、capability メソッドの既定戻り値が `Result` になる（audio を除く）。
- wave-1 の各 capability が「Core trait ＋ android/ios stub ＋ `platform/mobile/` facade ＋ ホストテスト
  （`Err(Unimplemented)` を公開契約越しに assert）」で着地する。FFI glue はソース走査ガードで封じ込める
  （既存 `apk_packaging.rs` / `audio_output_encapsulation.rs` と同パターン）。
- `crates/platform/README.md`（grouping doctrine 正本）に「Capability Scaffold＝2 leaf stub＋Flutter 契約形で
  ゲートの意図を満たす」「throw-by-default は taxonomy 同様に借りてよい（機構は不可）」を追記する follow-up
  が要る。
- **受容するリスク**: scaffold の契約形は Flutter 由来の初期見積りで、実機実装が入ると変わりうる（特に
  file picker の stream/path 返却、secure storage の鍵モデル）。変更コストを抑えるため、wave-1 は一発応答に
  絞り streaming（wave-2）と permissions（さらに後）を実装データが出るまで遅延させる。

## 関係

- ADR-0117（三層モデル・grouping doctrine）：本 ADR はその doctrine 下で初の breadth-first capability
  展開を定め、「昇格は 2 実装から / trait を先置きしない」をゲートの意図を満たす形に再フレームする。
- ADR-0068（投機 seam の戒め）／ADR-0069（contract は Core）：契約 Core 所有・実装 leaf の型を継続。
- ADR-0012（platform 等階級）：mobile 先行だが web/desktop への common 昇格を予測列として残す。
