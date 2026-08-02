---
status: accepted
---

# Native Accessibility は共通 module と薄い Platform Adapter に分ける

Hayate Core には AccessKit `TreeUpdate` の outbound projection と、`ActionRequest` を `InteractionIntent` へ写す inbound semantics が既にある一方、Desktop・Android・iOS の実 AT bridge はいずれも未実装である。三 platform を個別に実装して lifecycle・初期 tree・dirty update・focus・action forwarding を重複させず、これらを一つの platform-free な Native Accessibility module に集約する。各 leaf Platform Adapter に残すのは、winit window、Android GameActivity/View、iOS UIKit `UIView` と AccessKit platform adapter を接続する固有 glue だけとする。

Web Accessibility Mirror は native AT bridge と異なる読み取り専用 projection なので、この共通 module へ統合しない。設計は Desktop・Android・iOS を同時に確定し、実機実装と検証は Desktop → Android → iOS の順に進める。

AccessKit の activation / action callback から `ElementTree` へ同期アクセスしない。callback は要求を thread-safe mailbox へ enqueue して frame を起こし、次の App Host frame が単一 thread 上で mailbox を drain して Core へ適用する。activation は callback へ初期 tree を同期返却せず、次の frame で完全な `TreeUpdate` を生成して返す。通常の outbound update も Core commit 後の確定状態から生成する。これにより callback thread と `ElementTree` の間に共有可変 state や複製 snapshot を置かず、AccessKit が許容する「遅くとも次の display refresh までに initial tree を送る」契約を利用する。

native surface ごとに一つの `Native Accessibility Session` を App Host が所有する。Platform Front は OS 固有 adapter を構築し、session の outbound target として mount するが、その後の frame 位相を組み立てない。App Host は frame 前半で session mailbox を drain して action を `ElementTree` へ適用し、その結果の `Event Delivery` を同じ frame の consumer advance へ含める。Core commit 後には確定した layout・focus から accessibility update を生成し、mount 済み target へ渡す。これにより accessibility の input → consumer mutation → layout → outbound tree という順序を三 platform で共通化する。

MVP から node 単位の incremental `TreeUpdate` を送る。変更がない frame は Core の accessibility generation 比較だけで終了する。変更がある frame は accessibility tree を全 walk して最新 node 集合を構築するが、session が保持する送信済み `NodeId → Node` baseline と `Node::eq` で比較し、新規・変更 node だけを target へ渡す。子の追加・削除は children が変化した親 node の更新で表し、focus は AccessKit 契約どおり毎 update に最新値を含める。activation、再 activation、target / surface 再生成では baseline を破棄して完全な tree を送る。dirty node からの部分 walk は MVP に含めず、全 walk の実測が問題になった場合の後続最適化とする。

session lifecycle は `Detached → Dormant → InitialPending → Active` の明示的な状態機械とする。`Detached` は native target 未 mount、`Dormant` は target mount 済みだが AT 未 activation、`InitialPending` は activation を受けて次 frame の full update を待つ状態、`Active` は送信済み baseline から incremental update を送る状態である。deactivation は baseline と未送信 initial request を破棄して `Dormant` へ戻し、target / surface の破棄は `Detached` へ戻す。inactive 中に競合して届いた action は破棄する。activation 後から initial update 前までに届いた action は同じ frame で適用し、その結果を initial tree に含める。連続した activation request は AccessKit 契約に従って各要求へ完全な tree を生成する。

native target への delivery は `Applied | Inactive` を返す。session は生成した差分を staged state とし、`Applied` のときだけ送信済み baseline を進める。deactivation 競合などで target の update factory が実行されなかった `Inactive` は通常の lifecycle 結果であり、baseline を破棄して `Dormant` へ戻り、次の activation まで blind retry しない。AccessKit adapter の構築失敗や JNI / UIKit glue の致命的失敗は delivery の汎用 error に混ぜず、観測可能な target mount failure としてその surface の accessibility を無効化する。描画と App Host 自体は継続する。

Native Accessibility MVP の完了条件は bridge の接続だけではなく、screen reader の「読み上げ → focus → 起動／値入力 → scroll」基本ループが閉じることとする。outbound は role・name・value・bounds・focus に加え、element semantics に応じて `Focus` / `Click` / `SetValue` / `ScrollIntoView` action を明示する。inbound は ADR-0098 の同じ四 action を既存 `InteractionIntent` へ写す。Node identity は ADR-0098 で決定済みの専用 `AccessIndex` と `ElementId ⇄ AccessIndex` bimap を実装し、古い／不正な `NodeId` は action 入口で無視する。text-run node、`SetTextSelection`、selection の outbound 読み戻し、checked / expanded 等の richer state はこの bridge MVP と分ける。

Core は accessibility bounds を従来どおり論理 layout 座標で構築する。Platform Adapter は native container の base content scale（DPR）を session に渡し、session は root node の AccessKit transform として論理座標から container-relative physical pixel への scale を一度設定する。子 node の bounds は論理値のまま保ち、DPR 変更時は root transform の差分だけを送る。adaptive render scale は表示上の論理位置を変えない raster 品質制御なので accessibility transform に含めない。window / `UIView` / Android View の画面上の origin は leaf AccessKit adapter が扱い、Core や session は screen-space origin を持たない。

Core の element focus と native container focus を別の真実として保つ。`TreeUpdate.focus` は Core `Interaction` が所有する focused element（未選択なら root）を常に指し、window / view の blur で消去しない。window、`UIView`、Android View が foreground focus を持つかは leaf adapter が OS event を AccessKit の `process_event` / `set_focus` / UIKit lifecycle へ渡す。container focus を再取得したとき、AccessKit は保持されている element focus を再び公開する。session は container focus を第二の Core state として複製しない。

## Considered Options

- **全 native platform を `accesskit_winit` 一実装へ統一する** — AccessKit 自体は Desktop・Android GameActivity・iOS を扱えるが、Hayate は ADR-0087 / ADR-0114 で Android・iOS の direct platform binding と winit 非採用を決定済みである。Accessibility のためだけにその決定を崩すと、window/input/IME の ownership と矛盾するため採用しない。
- **各 Platform Adapter が lifecycle と semantics を個別所有する** — Core の outbound/inbound semantics に加えて三つの別実装が生まれ、Accessibility の locality と cross-platform leverage を失うため採用しない。
- **callback が thread-safe な accessibility snapshot を同期参照する** — initial tree を callback 内で返せる一方、`ElementTree` と別に同期対象となる意味 tree を持ち、commit 境界と focus の整合性を新たに管理する必要がある。最大一 frame の遅延で済む mailbox 方式より state と ordering が深くならないため採用しない。
- **Platform Front が session を所有し、`AppHost::tick` の前後で駆動する** — OS glue からは自然に見えるが、action drain・delivery・commit・outbound update の意味順序を各 front が再構築することになる。App Host の frame transaction の外側へ ordering を漏らすため採用しない。
- **変更があれば毎回完全な tree を platform adapter へ送る** — 実装は最小になるが、AccessKit 自身が新規・変更 node だけを含めることを性能上推奨しており、OS 側で未変更 node を処理・置換する費用が残る。`Node` の値比較だけで削減できるため MVP としても採用しない。
- **dirty node 集合から accessibility subtree だけを部分 walk する** — tree 構築費用まで削減できる一方、layout reflow、inline text 集約、透過 node、構造変更の ancestor reach を正しく閉包する別の invalidation 設計が要る。MVP の「そこそこの最適化」を越えるため後続とする。
- **delivery 前に incremental baseline を進める** — deactivation 競合で adapter が update を適用しなかった場合、sender と receiver が異なる前提 state になり次の差分が不正になるため採用しない。
- **target mount failure で App Host 全体を停止する** — accessibility failure を可視化できる一方、描画・入力まで使用不能にする必要はない。surface 単位の graceful degradation と観測を選ぶ。
- **既存 `TreeUpdate` をそのまま native adapter へ接続して MVP 完了とする** — outbound node が利用可能 action を宣言しておらず、NodeId も ADR-0098 の `AccessIndex` ではなく `ElementId` 直結の暫定実装なので、screen reader の基本操作ループを保証できない。bridge と基本 semantics を同じ MVP に含める。
- **Core が全 node の bounds を物理 pixel へ変換する** — Core の layout / hit-test の論理座標系へ DPR を混ぜ、DPR 変更で全 node が差分になる。root transform 一つで表せるため採用しない。
- **adaptive render scale を accessibility bounds に掛ける** — raster buffer の解像度低下は画面上の UI 位置・大きさを変えない。AT の hit-test 位置だけを縮める誤りになるため採用しない。
- **container blur 時に Core の element focus を clear する** — window を切り替えて戻ったときの入力 focus 復元を壊し、OS focus と document focus の ownership を混同するため採用しない。

## Consequences

- Native Accessibility の platform-free test surface を一つ持ち、initial tree、incremental update、focus、action forwarding、close の順序を platform SDK なしで検証する。
- leaf adapter の実機テストは OS AT への到達と platform event glue に集中する。
- Core の Interaction state ownership は変更しない。Accessibility inbound は引き続き `InteractionIntent` seam の consumer であり、第二の interaction state owner を作らない。
- activation / action callback の処理は最大一 frame 遅延する。callback は enqueue 後に App Host の唯一の wake seam から frame を要求し、idle 中でも要求を滞留させない。
- incremental baseline は native surface/session ごとに一つ保持する。Web Accessibility Mirror の poll cursor や Core の Interaction state と共有しない。
- lifecycle 遷移と callback の競合規則を platform-free test で固定し、leaf adapter の暗黙の active flag を正本にしない。
- target が `Inactive` を返した update は baseline へ反映しない。mount failure はログ／既存 observability seam から platform と category を識別できるようにする。
- text-run / selection accessibility を待たず、要素 node の基本 action loop を先に三 platform 共通で成立させる。
- Platform Adapter は mount / resize 時に base content scale を session へ報告する。DPR 変更は accessibility generation と独立に root transform の update を発生させる。
- leaf adapter の実機テストは container blur / focus 復帰後に同じ element focus が AT へ再公開されることを含む。
