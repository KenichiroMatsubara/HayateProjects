---
status: accepted
---

# Torimi Wire Contract を言語中立な正本から TypeScript / Rust / C++ へ投影する

Torimi の wire facts は現在、`@torimi/dev-server-contract`、`@torimi/protocol-handshake`、`@torimi/bundle` の TypeScript と、Hayate Android Native Host の複数 Rust module に分散している。TS 内では一部を共有できているが、Native Host は route、global 名、enum、JSON shape を値で複製しており、TS と Rust のテストが互いに異なる契約を検証したまま独立に green になれる。

新しい `@torimi/wire-contract` を Torimi wire facts の唯一の公開 package とする。既存 `@torimi/dev-server-contract` はこの package に吸収する。`@torimi/protocol-handshake` は version 比較と typed error という behavior module のまま残すが、global 名などの wire facts は `wire-contract` から参照する。`@torimi/bundle`、Web Host、Dev Server、Demo Endpoint と Native Host も同じ contract の各 projection を消費する。

contract に含めるのは、bundle / reload / log / manifest の route、reload message、`__torimiMount` / `__torimiProtocolVersion` / `__hayateHost` / `__tsubame` / `__hayateLog` の global 名、`LogLevel` / `LogSource` enum、`LogEntry` / `LogBatch` / `DemoManifest` の wire shape である。protocol version 比較、reconnect backoff、fetch timeout、log flush interval / buffer capacity、boot / reload 状態機械は wire fact ではないため含めず、それぞれの behavior module に残す。

言語中立な正本は `Torimi/wire-contract/spec/` の JSON vocabulary manifest と JSON Schema とする。vocabulary は route / message / global / enum を、schema は HTTP JSON payload の field・型・必須性・未知 field の互換規則を定義する。Torimi-owned の単一 Node ESM script が committed TypeScript projection（定数・union・interface・validator）、Rust projection（定数・enum・`serde` DTO）、C++ projection（global / route / message 名の `constexpr` 定数）を決定的に生成する。MVP では外部 codegen framework を追加せず、入力順序に依存しない安定した field / declaration 順と改行を generator 自身が保証する。runtime に schema engine や generator は持ち込まない。CI の `check:wire-contract` は同じ script で再生成して working-tree diff を検出し、さらに各 projection の compile / test を実行する。正本だけ更新していずれかの projection を更新し忘れる変更は拒否する。Device Log の additive-only contract は未知 field を許可する schema / deserializer として表現する。

現時点の Rust projection は、唯一の Rust consumer である Android Native Host の `Hayate/crates/platform/mobile/android/src/generated/torimi_wire.rs` へ committed generated artifact として出力する。Hermes JSI C++ も global 名を直接読み書きするため、`Hayate/crates/platform/mobile/android/cpp/generated/torimi_wire.hpp` に定数だけの committed C++ projection を出力する。payload shape は Rust 側だけが decode / encode するため C++ に DTO を複製しない。生成元と generator は Torimi が所有し、両出力には generated header を付けて手編集を禁止する。これにより `Hayate → Torimi` の新しい Cargo dependency を作らず、Android の手書き Rust / C++ module は生成済み wire facts だけを import する。iOS Native Host という二つ目の Rust consumer が実装された時点で、shared Rust crate / neutral placement への昇格を再評価する。

正本は一つでも runtime compatibility policy は channel ごとに維持し、全体 `TorimiWireContractVersion` は新設しない。App Bundle と Hayate decoder は既存 `Protocol Version` の完全一致、Device Log は version token なしの additive-only、Demo Manifest は必須 field を検証しつつ未知 field を許容して配信物を release lockstep、reload route / message と global seam は生成定数＋package major / 協調リリースで扱う。被害の小さい log / manifest の additive change を厳格な app mount rejection に連動させない。

generated 型は wire boundary の値に限定する。route / global / message、log enum / entry / batch、manifest DTO は生成物を直接使い、`DevServerTarget`、`BootPlan`、`BootError`、`ProtocolMismatch`、Device Log の buffer / retry 状態、reload controller は手書き domain / behavior のまま残す。JSON / JS global と domain state の間には明示的な conversion / validation を置く。wire そのものを蓄積して送る Device Log は generated `LogEntry` を直接保持できるが、Demo Manifest から接続先や boot 手順を作る処理は generated DTO から `BootPlan` へ変換して意味検証する。

JS global seam は、生成物が global / property 名と TypeScript 型を供給し、各 consumer が利用前に一度だけ構造検証する。Web Host は `__torimiMount` が function であることを呼び出し前に確認する。Native Host は bundle eval 直後に `__tsubame` が object で、`pumpFrame` と `stop` が function であることを確認し、最初の frame まで shape failure を遅延させない。失敗は明示的な boot failure として報告する。C++ の JSI 値検査は generated name constants を使う手書き boundary code とし、generator / wire-contract に JSI dependency を持ち込まない。`__hayateHost` は Native Host だけが eval 前に注入できる予約 global なので、その存在を native target の discriminator として使い、広大な `RawHayate` method surface の全面 runtime validation は行わない。

generated TypeScript validator は `unknown` を受け取り、例外を投げず、`{ ok: true, value: T } | { ok: false, issues: WireIssue[] }` を返す。`WireIssue` は `path`、`expected`、`actualType` のみを持ち、入力値そのものは診断へ複製しない。consumer はこの wire-level result を HTTP 400、boot failure、Device Log 等の domain error に変換する。Rust は generated `serde` DTO の decode `Result` を boundary で domain error へ変換し、TypeScript と同一の error class / result shape まで生成しない。

Device Log の `LogLevel` と `LogSource` は open vocabulary とする。wire 上では空でない string として未知値も受理・保持し、generated projection は既知値の定数と `isKnownLogLevel` / `isKnownLogSource` 相当の判定を提供する。domain 変換後は既知値を通常分類し、未知値を `unknown` 表示へ退避する。新しい producer の語彙追加で旧 Dev Server が batch 全体を拒否しないためである。一方、reload message kind のように値が分岐動作を決める語彙は closed とし、未知値を契約違反として拒否する。vocabulary manifest は各語彙の `open` / `closed` を明示する。

`Torimi/wire-contract/fixtures/` には schema / generator から作らず人手でレビューする共有 fixture corpus を置く。各 DTO は最小 valid、全 field valid、未知 field 付き valid、必須 field 欠落、型違い、未知 enum の case を持ち、invalid fixture には期待する失敗 path を添える。generated TypeScript validator と generated Rust DTO の双方が同じ accept / reject 判定になることを CI で確認する。payload を扱わない C++ projection は fixture test の対象にせず、generated header の compile test に限定する。

Device Log の受信 behavior は、strict な `validateLogBatch` で batch 全体を一括拒否せず、部分救済を維持する。body、`deviceLabel`、`entries` 配列という envelope 自体が不正なら HTTP 400 とするが、各 entry は generated `validateLogEntry` で個別検証し、不正 entry だけを破棄して有効 entry を sink へ渡す。不正 entry の `seq` は重複排除 watermark を進めない。全 entry が不正でも HTTP 204 で受理を確定して producer の永久再送を止め、Dev Server には入力値を含めない validation summary を request ごとに一度だけ出す。strict な `validateLogBatch` は完全な wire 値を必要とする consumer と fixture parity test に残す。この救済判断は Device Log behavior であり、schema 自体を曖昧にしない。

移行は互換 layer を置かず、一つの atomic change で行う。全 workspace consumer を `@torimi/wire-contract` へ切り替え、`@torimi/dev-server-contract` package は削除する。`@torimi/protocol-handshake` からも wire 定数の定義・re-export を除き、version 比較と typed error だけを公開する。Torimi は未公開段階で既存利用者との後方互換を維持する必要がなく、旧入口を残すことによる二つの正本・不要な migration surface を避ける。

## Considered Options

- **`@torimi/dev-server-contract` を名前だけ維持して native globals まで追加する** — `__hayateHost` や bundle registration は Dev Server を通らず、package 名と bounded context が一致しなくなるため採用しない。
- **`@torimi/protocol-handshake` へ全契約を集約する** — log / manifest / reload shape は version handshake ではなく、behavior package が汎用 wire 正本を兼ねるため採用しない。
- **Rust mirror を現状どおり手書きし cross-language fixture test だけ追加する** — literal と型の重複は残り、新 field 追加時に片側の更新漏れをコンパイルで防げないため採用しない。
- **TypeScript source を正本として Rust だけ生成／検査する** — TS の型表現と module 構造が言語中立 contract を兼ね、Rust projection が TypeScript compiler semantics に従属するため採用しない。
- **protobuf 等の binary IDL に移行する** — 現行 transport は JS globals・URL literals・JSON HTTP payload であり、binary encoding を導入せずとも JSON vocabulary / Schema で全 wire facts を表せるため採用しない。
- **今すぐ Torimi-owned Rust crate を作り Android adapter から path dependency する** — 単一 consumer のための crate を先置きし、Hayate から Torimi への逆向き build dependency も追加するため採用しない。二つ目の native consumer ができるまでは generated leaf module とする。
- **Rust projection を Torimi 配下に置いて `include!` する** — Cargo manifest 外の相対 path を compile-time contract にし、package 単体 checkout / build を壊しやすくするため採用しない。
- **全 Torimi wire に単一 runtime version handshake を課す** — Device Log の additive field 追加まで App Bundle mount 拒否へ波及し、channel ごとの failure impact を無視するため採用しない。
- **generated DTO を Native Host の lifecycle / domain model として全面利用する** — schema field 名や additive compatibility が boot state machine の内部表現まで支配し、transport 変更と behavior 変更を結合するため採用しない。
- **generated constants だけを使い payload struct / enum は手書きのまま残す** — literal drift は減るが optionality、serde field name、enum value の cross-language drift が残るため採用しない。
- **TypeScript / Rust だけを生成し Hermes JSI C++ の global 名は手書きのまま残す** — Native Host の実際の producer / consumer に同じ seam 名の第三の mirror が残り、Rust が追従しても C++ だけ独立に drift できるため採用しない。
- **C++ にも全 JSON DTO を生成する** — C++ は現状 payload の decode / encode を担当せず、未使用の shape と validator を増やすだけなので採用しない。C++ projection は C++ が直接触る名前と token に限定する。
- **既製の multi-language codegen framework を導入する** — 現行 contract は少数の literal / enum / 単純な JSON DTO で、TypeScript / Rust / C++ の必要な投影範囲も異なる。MVP では依存と framework 固有中間表現を増やすほどの複雑性がないため採用しない。
- **global 名だけを共有し値 shape は利用時の例外に任せる** — `__tsubame` の欠落や誤型が bundle eval ではなく最初の frame で初めて発覚し、boot failure と runtime frame failureを混同するため採用しない。
- **generator から JSI 固有の C++ validator まで生成する** — language-neutral contract を Hermes / JSI API と結合し、単純な構造検査のために generator の責務を広げるため採用しない。
- **`RawHayate` の全 method surface を起動時に反射検証する** — Native Host 自身が生成・注入する予約 global に大きな検査コストと二重のinterface記述を加える一方、実装不整合は native/bundle の protocol version 検査で扱えるため採用しない。
- **TypeScript validator を type predicate の boolean だけにする** — 拒否理由と field path が失われ、Dev Server / Web Host が独自に再検査することになるため採用しない。
- **TypeScript validator が契約違反で例外を投げる** — HTTP input や eval 後 global のような不正値を通常の境界分岐ではなく例外制御へ押し込み、consumer ごとの failure mapping を難しくするため採用しない。
- **入力値を validation issue に含める** — bundle URL や log message を診断オブジェクトへ複製し、ログへの再出力や機微情報漏洩を招きやすいため採用しない。
- **schema から test fixture も自動生成する** — schema の読み違いを generator と fixture generator が共有すると全 projection が同じ誤った結果で green になれるため採用しない。
- **言語ごとに独立した fixture を持つ** — TypeScript と Rust がそれぞれの実装に都合のよい例だけを検査でき、projection 間の accept / reject drift を発見できないため採用しない。
- **Device Log の level / source を strict enum にする** — 新しい producer が語彙を追加しただけで旧 Dev Server が LogBatch 全体をdecodeできず、field追加を許すadditive-only方針をenum値が迂回して破るため採用しない。
- **すべての vocabulary をopenにする** — reload message kindなど、未知値に安全な意味を与えられない制御語彙まで受理して挙動を曖昧にするため採用しない。
- **一件でも不正なentryがあればLogBatch全体をHTTP 400で拒否する** — producerが同じbatchを再送し続け、後続の有効ログまで永久に詰まらせるため採用しない。
- **部分救済のためLogEntry schema自体をoptionalだらけにする** — wire上の正しいentryの定義を弱め、sinkへ半端な値が到達するため採用しない。strictなentry validatorを個別適用して救済する。
- **旧 `@torimi/dev-server-contract` と wire 定数の旧 re-export を一時的な互換 façade として残す** — 未公開段階で保護すべき既存利用者がおらず、二つの入口と削除予定コードだけを増やすため採用しない。

## Consequences

- wire change は Torimi-owned contract の一箇所から始め、TypeScript / Rust / C++ の projection を同じ変更で更新する。
- `@torimi/wire-contract` は transport / host / framework behavior に依存しない leaf package とする。
- Native Host の Android 固有 Rust / C++ module は generated wire values / DTO を消費し、同じ literals や serde field names を再宣言しない。将来 iOS も同じ Rust projection を使う。
- local tuning constants と lifecycle policy を「共有できそう」という理由だけで wire contract へ入れない。
- generated TypeScript / Rust / C++ に手編集を加えず、必要な表現変更は spec または generator に戻す。
- CI は generated Rust output が Android crate 単体で `serde` compile / test できることと、generated C++ header が Hermes JSI translation unit から compile できることも確認する。
- generator は同一入力からbyte-identicalな出力を生成し、`check:wire-contract` は再生成後の差分が空であることを要求する。
- schema / generator は各 channel の unknown-field と required-field policy を projection の validator / deserializer に反映する。
- conversion test は wire shape の妥当性と domain policy（例: manifest の URL / 空一覧）を別々に検証する。
- TypeScript consumer は generated validator の `WireIssue` をそのまま domain error とせず、各 channel の failure vocabulary へ明示的に変換する。
- shared fixture corpus は generator とは独立に人が期待結果を記述し、TypeScript / Rust の accept / reject parity を検査する。
- Device Log fixture は未知の非空 level / source を valid として含み、closed vocabulary の未知値は invalid として含める。
- Device Log handler は envelope failure と entry failure を分け、entry failure では有効entryを救済しつつ不正entryをacknowledge-and-dropする。
- global seam の consumer は generated name constants を使って一回限りの構造検証を行い、shape mismatch を利用時の偶発的例外ではなく boot failure として分類する。
- migration PR は workspace 内の全 import、package dependency、README / ADR reference と generated native projection を同時に更新し、旧 contract package や旧 wire 定数入口を残さない。
