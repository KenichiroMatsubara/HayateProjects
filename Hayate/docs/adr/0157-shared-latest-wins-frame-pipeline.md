---
status: accepted
---

# Latest-Wins Frame PipelineをWeb/Native共通deep moduleとする

`Committed Frame`の受理からraster完了までのoverload制御を、Web/Nativeそれぞれに実装せず、一つのplatform-freeなRust module `Latest-Wins Frame Pipeline`が所有する。ADR-0128が認めたWeb/Nativeの非対称はthread、Worker、wake、surface、GPU完了通知などの実行機構に限定し、latest置換、dirty union、lifecycle順序、failure semanticsをplatformごとに変えることは認めない。

## Decision

`Latest-Wins Frame Pipeline`は次の不変条件を単一のimplementationとして所有する。

- raster中のactive commandに対して、置換可能なpending `Committed Frame`を最大一件だけ保持する。
- 新しいframeはpendingの`Scene Snapshot`と`Layer Topology`を最新へ置換し、置換されるframeだけが持つcontent/chrome dirty、topology変更、`Layer Presentation`の未反映workをunionする。
- resize、surface lost、surface rebuildなどのlifecycle commandはframeとcoalesceせず、前後の相対順序を維持する。lifecycle barrierを跨いでframeを置換しない。
- renderer実行またはsurface操作がterminal failureになった後は、新しいframeを受理せず、別rendererへのruntime fallbackやworker restartを行わない。
- accepted、coalesced、dropped、active、pending、failureを共通のperformance observabilityとして公開する。

このmoduleは`std::thread`、`Mutex`、`Condvar`、`JoinHandle`、JavaScript `Worker`、Promise、Choreographer、`requestAnimationFrame`、surface object、renderer固有resourceをinterfaceにもimplementationにも含めない。狭いexecution seamから「実行可能になった」「commandが完了した」「terminal failureになった」という事実だけを受け、次に実行すべきcommandを決める。

Native execution adapterは`std::thread`とwake primitiveを使って同じmoduleを駆動する。現在の`RasterThread`はqueue policyを所有するmoduleではなく、このNative adapterへ縮小する。Web execution adapterはbrowser event loopとWorker/OffscreenCanvas、WebGPU completionを使って、WASMへコンパイルされた同じmoduleを駆動する。TypeScript transport、Worker host、renderer adapterは独自のlatest-wins queue、dirty merge、lifecycle ordering、failure recoveryを持たない。

Platform Frontは引き続きChoreographerまたは`requestAnimationFrame`との結線と、一vsync内のwake集約だけを所有する。`Latest-Wins Frame Pipeline`はcommit済みframe以降を所有するため、OS/browserのframe clock機構を共通化しない。Skia、Vello、tiny-skiaのencode、batching、surface処理も各`Scene Renderer` implementationに残し、renderer差をframe policyへ漏らさない。

共通moduleのtest surfaceは、同じframe/lifecycle/completion/failure列から、最新snapshot、dirty union、barrier順序、bounded pending、terminal state、観測値が一致することとする。Native/Web adapterのtestはwake、transport、surface ownership、完了通知だけを検証し、policyの期待値を複製しない。cutover後は旧`RasterThread`内のqueue policyとWeb固有の代替policy、runtime flag、fallback経路を削除する。

## Considered Options

- **Web専用のFrame Admissionを追加する**: Nativeのlatest-wins、dirty union、lifecycle順序をWebへ再実装し、修正と性能最適化が二重になるため不採用。
- **共通traitだけを置き、policy implementationはplatformごとに持つ**: interfaceは共通でも本質的な複雑性とtestが重複するshallowなseamになるため不採用。
- **現在のNative `RasterThread`を具象thread機構ごとWASMへ移植する**: SharedArrayBuffer、COOP/COEP、WASM atomics/thread buildを全Web配信環境の要件にし、browserのWorker/WebGPU ownership差まで共有moduleへ持ち込むため現時点では不採用。将来cross-origin isolationをHayate Canvas Modeの必須deployment contractにする場合は、別ADRでexecution adapterを統合できる。
- **Nativeだけlatest-winsとし、Webはengine全体をWorkerへ移すだけにする**: DOM main threadは空いても、Web側のframe backlog、GPU backpressure、lifecycle順序が別規則または無規則になるため不採用。

## Consequences

- optimization semantics、implementation、contract testの正本は一つになり、NativeとWebの性能改善は同じmoduleに蓄積される。
- 二つのadapterは、Native threadとWeb Worker/event loopという実在するvariationだけを吸収する。どちらかを削除してもframe policyは失われない。
- WebでDOM main threadをrasterから隔離するには、引き続きOffscreenCanvas＋Workerが必要である。本ADRだけで同期的なWASM encode中のevent-loop停止が解消されるわけではない。
- Nativeと完全に同じproducer/raster二thread形をWebで採るかはdeployment contractの判断として残るが、どちらを選んでもframe policyを別実装しない。
- ADR-0156のperformance acceptance policyに従い、共通化はrepresentative/stress workloadで検証する。改善不足を理由に旧Web/Native policyをruntime fallbackとして残さない。

## 関係

- **generalizes** ADR-0154のRaster Handoff以降のlatest-wins、dirty union、lifecycle順序をWeb/Native共通moduleへ拡張する。ADR-0154のAndroid Choreographer駆動はNative execution adapterの決定として維持する。
- **narrows** ADR-0128のWeb/Native非対称を実行機構だけに限定する。OffscreenCanvas＋単一WorkerというWeb配置は維持するが、optimization semanticsの非対称は本ADRが上書きする。
- **depends on** ADR-0152（Layer Presentation transaction）、ADR-0153（Immutable Scene Snapshot / Layer Scene）、ADR-0155（Render Resource Residency）、ADR-0156（性能acceptance policy）。
