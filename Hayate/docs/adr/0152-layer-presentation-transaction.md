# Layer Presentation transaction と backend adapter 境界

**Status: accepted**

Layer Presentation は `hayate-layer-compositor::LayerPresentation` が所有する stateful な transaction とする。
入力は committed frame の Scene Snapshot・Layer Topology・scroll geometry であり、単一入口が prepare → execute → commit を進める。

所有権は次のように分ける。Committed Frame は Core commit が確定した renderer-neutral な frame facts と owned `Scene Snapshot` を持つ。`Layer Scene` は Scene Snapshot と layer identity を共有所有する zero-copy projection であり、抽出済み SceneGraph を作らない（ADR-0153）。Layer Presentation は placement、適用済み revision、cache ledger、未反映 dirty work を retained state として所有するが、layer scene の copy は保持しない。backend adapter は texture・Pixmap・GPU resource など renderer 固有資源だけを所有する。

- **prepare** は presentation 固有の整合性を backend work 前に検証する。new・dirty・構造変更 layer は最新 Scene Snapshot から O(1) の Layer Scene を作り、clean layer は retained entry の存在だけを O(1) で確認する。Core raster bounds など必要な frame facts の欠落は統一された `InvalidFrame` とする。
- **execute** は shared module が作った raster job と placement plan を backend adapter が処理する。texture/Pixmap raster、quad composite、Canvas blit は adapter に残す。
- **commit** は raster、composite、blit の全成功後だけ cache ledger と LRU を更新する。失敗時に shared ledger は更新しない。stale layer と budget eviction は shared ledger が決めた同じ layer 集合を adapter に discard させる。

clean frame は Layer Scene の生成も scene walk も行わない。Core は前回の immutable placement table と raster bounds を構造共有し、Layer Presentation はその table を retained state へ適用する。dirty・追加・構造変更された layer は zero-copy Layer Scene を一度だけ生成する。execute が失敗した場合は retained revision と cache ledger のどちらも更新せず、content・chrome・placement rebuild の未反映 workを失わないが、Layer Presentation自身はretryをscheduleしない。削除された layer の retained state と backend resource は transaction が成功した commit 時にだけ破棄する。

Layer Presentationは`InvalidFrame`とrenderer adapter failureを区別してRender Hostへ返し、続行可否は既存のRenderer Selection Policy / Fatal Renderer Failureが決める（ADR-0150）。Committed Frameの契約違反を別rendererで隠したり自動retryしたりしない。選択済みskia-safeのraster / composite / surface / context failureはterminalであり、別rendererへのfallbackも次frame retryも行わない。AndroidのTerminateWindowなど正常なsurface lifecycleはtransaction開始前に扱い、surface rebuild後に最新frameを提示する。budget evictionはfailureではなく、次に必要になったとき通常のraster workとなる。他rendererで明示的にrecoverableな分類をRender Host policyが許可した場合だけ、保持された未反映workを最新Scene Snapshotへ適用して再提示できる。

placement の更新要否は Committed Frame の Layer Topology から決め、Layer Presentation が Scene Snapshot 全体を diff して構造変更を再発見する設計は採らない。Core は構造・geometry・transform が不変な commit では placement table を共有し、scroll affine は compositor 入力から該当 placement だけへ反映する。実際に適用した placement と revision は Layer Presentation が retained state として所有する。

SceneGraph の構造検証は Core の incremental validator を唯一の正本とし、初回は全体、以降は変更 subtree だけを検証する（ADR-0148）。full structural validation は Debug・CI・調査版で有効にし、production release では compile-time に除外する。Layer Presentation はこの構造検証を重複して行わないが、renderer 資源を安全に扱うための O(1) の presentation 整合性 check は production にも残す。いずれの検証失敗も execute 前に transaction を中止し、retained state を更新しない。

tiny-skia・Vello・skia-safe と web・desktop・Android・iOS の production adapter はすべて同じ transaction を使う。これにより backend ごとの raster/composite 実装を残したまま、frame validation、cache ownership、scroll-aware planning、failure semantics の所在を一箇所に固定する。旧全面描画 interface、runtime flag、copied extraction fallback は production host contract から削除する。
