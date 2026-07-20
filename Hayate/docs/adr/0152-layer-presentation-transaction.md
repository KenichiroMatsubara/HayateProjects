# Layer Presentation transaction と backend adapter 境界

**Status: accepted**

Layer Presentation は `hayate-layer-compositor::LayerPresentation` が所有する stateful な transaction とする。
入力は committed frame の SceneGraph・layer list・Core raster bounds・dirty set・scroll geometry であり、単一入口が prepare → execute → commit を進める。

- **prepare** は全 non-root layer の sub-scene と Core raster bounds を backend work 前に検証する。欠落は統一された `InvalidFrame` とする。
- **execute** は shared module が作った raster job と placement plan を backend adapter が処理する。texture/Pixmap raster、quad composite、Canvas blit は adapter に残す。
- **commit** は raster、composite、blit の全成功後だけ cache ledger と LRU を更新する。失敗時に shared ledger は更新しない。stale layer と budget eviction は shared ledger が決めた同じ layer 集合を adapter に discard させる。

tiny-skia を最初の end-to-end adapter とし、web backend は `TinySkiaLayerPresentationAdapter` を通す。Vello と skia-safe は後続移行で同じ transaction を使う。これにより backend ごとの raster/composite 実装を残したまま、frame validation、cache ownership、scroll-aware planning、failure semantics の所在を一箇所に固定する。
