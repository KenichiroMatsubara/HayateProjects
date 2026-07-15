---
status: accepted
---

# wire consumer は二相 frame contract で delivery 由来 mutation を同一フレームに反映する

App Host の `delivery → consumer flush → render` 順序は Tsubame Hayate Renderer の wire projection でも維持する。ただし、WASM から JS の handler を同期 callback し、その handler が同じ WASM object へ mutation を再入させる形は、Rust の可変 borrow と wire 実装を衝突させるため採用しない。wire frame は preparation と commit/present の二相に分け、preparation が drain 済み Event Delivery を JS へ返していったん Rust の呼び出しを終了し、JS が handler 実行と mutation flush を完了した後に commit/present を呼ぶ。in-process consumer は同じ二相を App Host 内で連続実行し、従来の単一 `tick(timestamp_ms)` projection を保てる。

二相の順序は呼び出し側の慣習にせず、App Host が `Idle` / `Prepared(frame_id)` の状態機械として所有する。`prepare_frame(timestamp)` は新しい `frame_id` と delivery batch を返して `Prepared` へ遷移し、対応する `commit_frame(frame_id)` だけが render/present して `Idle` へ戻す。二重 prepare、prepare なし commit、古いまたは不一致の `frame_id` は明示的な frame protocol error とする。Web adapter はこの interface を wire へ投影するだけで、独自の順序状態や暗黙補正を持たない。in-process `tick` もこの同じ状態機械を内部で駆動する。

consumer handler または mutation packet 生成が失敗した場合、wire consumer は対応する `abort_frame(frame_id)` を明示的に呼ぶ。Tsubame は handler dispatch と mutation packet 生成を一つの `try` 境界で行い、成功時にだけ packet 全体を WASM へ flush してから commit する。失敗時は未送信 packet を破棄して abort し、App Host は `Prepared` から `Idle` へ戻るが render/present は行わず、直前に成功した frame を表示し続ける。例外は既存の host error reporting へ伝播させる。部分的に mutation を flush してから abort することは許さない。また `Prepared` のまま次の prepare が来ても自動回復せず、frame protocol error とする。

wire mutation packet は validate-then-apply の二段階で処理する。第1段階は packet 全体を検証・decode して semantic mutation 列を生成し、`ElementTree` を変更しない。第2段階だけが検証済み mutation 列を順序通り適用する。decode/validation failure は tree 未変更のまま frame を abort できる。適用開始後の panic または Core invariant 違反は回復可能な abort ではなく terminal Core failure とし、一般的な rollback engine や `ElementTree` clone は導入しない。

`commit_frame` の失敗は状態遷移の異なる二系統に分類する。`FrameProtocolError` は prepare なし commit や `frame_id` 不一致などの呼出規約違反であり、App Host は `Prepared` を維持して、正しい commit または明示 abort による修復を要求する。`FrameExecutionError` は render/present の実行失敗であり、同じ frame を再実行せず `Idle` へ戻し、直前に成功した frame を表示したまま既存の Render Host failure policy へ渡す。App Host は失敗を分類するが、renderer fallback または terminal failure の判断は所有しない。

App Host が所有するのは platform 共通の frame transaction であり、DOM、Canvas、IME、clipboard、adaptive render scale の orchestration ではない。Web Adapter は Web 向けの深い `prepare_frame` / `commit_frame` façade を提供する。Web `prepare_frame` は pending resize、pointer input、edit input、font mailbox、pending paste を既定順で drain してから App Host の prepare を呼ぶ。Web `commit_frame` は App Host の commit/present 後に IME 同期と次フレーム用 render-scale 判定を行う。JS へ個別 drain API や順序制御を公開せず、`Idle` / `Prepared(frame_id)` の正本と protocol validation は App Host だけが持つ。Web Adapter は platform 固有の前後処理を投影するが、独自の frame protocol state は持たない。

Core の frame commit は renderer-ready かつ platform-free な不変 view `CommittedFrame` を返す。これは `SceneGraph`、frame layer 順序、content/chrome dirty layer、scroll layer の compositor 入力、pending visual work の有無だけをまとめる。App Host は `PresentTarget::present(&CommittedFrame) -> Result<(), FrameExecutionError>` を呼び、`ElementTree` 自体を Render Host へ公開しない。Web の layer compositor は `CommittedFrame` の scroll 入力を backend 用 geometry へ変換し、background、surface size、render scale など platform 固有値は PresentTarget 側で補う。

## Considered Options

- delivery 由来 mutation を次フレーム反映とする: 現行実装に近いが、入力結果を同じフレームへ畳む App Host の順序不変条件を捨てるため不採用。
- WASM → JS → 同一 WASM object の同期再入: 単一呼び出しの見かけは保てるが、再入可能な可変所有を wire interface に要求するため不採用。

## Consequences

- ADR-0117 の `DeliverySink` は in-process projection では同期 callback、wire projection では二相間を運ぶ delivery batch として具体化される。
- wire consumer が preparation と commit/present の間に handler 実行と mutation flush を完了しない限り、その frame を commit してはならない。
- consumer failure は `abort_frame(frame_id)` で明示的に閉じる。abort された frame は描画せず、mutation packet は全体適用または未適用のどちらかでなければならない。
- wire packet の原子性は全体検証後にだけ適用することで保証し、適用済み mutation の巻き戻しは提供しない。
- protocol error は修復可能な prepared frame を残す一方、render/present failure は frame を閉じて再試行しない。
- Render Host の既存 renderer failure policy は維持され、App Host へ fallback 判断を重複させない。
- 当初案の「Web frame orchestration 全体を App Host に吸収する」は採用しない。共通 transaction は App Host、platform 固有の ingress/egress は Adapter façade に集約する。
- 現行の `PresentTarget::present(&SceneGraph)` は必要な frame metadata を運べないため廃止し、`ElementTree` を渡す近道も採用しない。
