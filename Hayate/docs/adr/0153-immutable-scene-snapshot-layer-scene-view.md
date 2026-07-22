---
status: accepted
---

# Immutable Scene Snapshot と zero-copy Layer Scene

Core の mutable な retained scene を renderer へ直接貸し出さず、frame commit は構造共有された不変 value `Scene Snapshot` を確定する。`Committed Frame` はこの snapshot と `Layer Topology` を所有し、後続の lowering や UI thread の進行から独立した寿命で Render Host と Raster Thread へ渡す。SceneGraph module は persistent structural sharing、parent / Element Anchor index、layer traversal、change journal を implementation に隠し、snapshot の生成と共有を caller に再実装させない。

layer raster の入力は、subtree を新しい SceneGraph へ複製した値ではなく、Scene Snapshot と layer identity を共有所有する zero-copy `Layer Scene` とする。Layer Scene の canonical walk はその layer に属する node だけを訪れ、子孫の別 layer を除外し、合成時に適用する外側 transform・clip・scroll affine を内容へ焼き込まない。この規約は SceneGraph module が一度だけ実装し、各 Scene Renderer adapter は独自の抽出規約や scene walk を持たない。

Layer Presentation は前回成功した placement、適用済み revision、cache ledger、未反映 dirty workを保持するが、抽出済みSceneGraphまたはLayer Sceneのcopyをretained stateにしない。dirty layerのraster時だけ最新snapshotからO(1)のLayer Sceneを作り、clean frameではview生成もscene walkも行わない。Raster Threadへのhandoffはsnapshot handleをO(1)で共有し、UI側の次frame mutationから隔離する。

cutoverでは既存のsub-scene copy経路を一時的なtest oracleとして使い、同じfixtureのDrawOp、placement、pixel結果をzero-copy Layer Sceneと比較した。parity成立後は旧抽出関数、旧host interface、runtime flag、fallback、旧経路testを削除し、productionとtestのlayer projectionを同じLayer Scene interfaceへ統一した。

## Considered Options

- layer ごとに抽出した SceneGraph copy を retained にする案は、canonical scene と layer scene の二重表現、copy、同期、peak memory を Layer Presentation へ持ち込むため不採用。
- lifetime 付き borrowed subgraph は、asynchronous Raster Thread と frame coalescing に借用制約を漏らすため不採用。
- renderer ごとに layer extraction を実装する案は、walk 規約と不具合を全 adapter へ複製するため不採用。

## Consequences

- SceneGraph module は小さな snapshot / layer projection interface の裏に、persistent storage、index、差分追跡、canonical traversal の複雑さを集約する。
- mutable scene の最初の変更で全 index を複製する実装は最終形とせず、変更量に比例して共有を外す persistent storage を採る。
- Scene Renderer と parity test は同じ Layer Scene interfaceを通じて観測し、内部indexやstorage表現を直接検査しない。
- Committed Frame、Layer Presentation、Raster Handoff はSceneGraphのnode storage詳細を知らない。
- cutover後のproductionとtest surfaceに旧sub-scene copy経路を残さない。
