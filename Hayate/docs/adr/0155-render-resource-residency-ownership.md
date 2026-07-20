---
status: accepted
---

# Render Resource Residencyの所有とbudget policy

renderer固有資源の所有を、選択済みScene Renderer instanceごとの`Render Resource Residency`へ集約する。SkTypeface、SkTextBlob、SkImage、layer textureなどの具体資源はrendererとRaster Threadの寿命に揃え、thread-localまたはprocess-global cacheに置かない。renderer選択後に存在するresidencyは一つだけとし、renderer終了時に全poolを一括破棄する。

Coreはfont bytes、image bytes、shaped textなどrenderer-neutralなsource resourceとstable identityを所有する。`Font Instance`はfont Blob、face index、variation、synthesisを、`Text Run`はFont Instance、font size、glyphと位置、decorationをinternしたimmutable valueとする。raster結果を変える変更は必ず新しいgenerational IDになり、同一のinterned valueはframe間で同じIDを共有する。renderer lookupは固定長IDだけを使い、variation座標やglyph列をframeごとにclone・hashしてidentityを再構築しない。CoreはSkia/Vello/tiny-skiaの具体objectとresidency policyを知らない。

IDはScene Snapshotが参照している間再利用せず、pointer addressをidentityにしない。intern tableは未参照valueをsnapshot解放後に回収でき、thread handoffとwire projectionは固定長IDを運ぶ。content hashを外部identityにしてcollision処理と再hashをcallerへ要求する設計、およびinterningなしの単純採番で同一内容の共有を失う設計は採らない。

Render Hostはplatformのcomposition rootからdevice memory class、surface size、OS memory pressureを受け、resource budget policyを決める。CPU-backed resourceとGPU resourceはbyteの意味、再構築cost、surface lifecycleが異なるため、一つの単純LRUへ混ぜず、同じpolicy配下の別poolとして扱う。surface lossはGPU poolだけを破棄でき、memory pressureはpool別budgetを縮小できる。Platform Frontはframe schedulingだけを所有し、resource budgetを持たない。

Layer Presentationはlayer revision、利用状況、logical byte ledgerを保持し、budget判断へlayerの情報を供給するが、renderer固有resourceを所有しない。Render Resource Residencyはhit、miss、eviction、resident bytes、rebuild costを共通のperformance観測経路へ出す。

## Considered Options

- thread-local cacheは寿命と個数がrenderer instanceから見えず、thread追加時に重複するため不採用。
- CPU/GPU resourceを一つのglobal byte LRUへ入れる案は、memory domainと再構築costの差を失うため不採用。
- Platform Frontがbudgetを持つ案はscheduleとresource lifetimeを混在させるため不採用。
- Coreがrenderer objectを所有する案はrenderer-neutral sourceと具体資源を混同するため不採用。
- content hashを外部identityにする案はcollision処理と再hashをinterfaceへ漏らすため不採用。
- interningなしの単純採番は同一内容を再生成したときにrenderer resourceを共有できないため不採用。

## Consequences

- SkiaのFontMgr、typeface、text blob、image、layer textureは一つのrenderer-scoped ownership storyに収束する。
- stable resource identityの追加は全Scene Rendererが共有でき、可変長cache keyのframe-local allocationを除去する。
- Text Runをimmutable shared valueにすることでScene Snapshotとscene nodeもglyph dataを構造共有できる。
- budget testはpool別resident bytes、eviction順、surface loss、memory pressureをRender Resource Residencyのinterfaceから検証する。
- renderer adapterはresource lookupとrasterを同じresidencyへ委ね、独自cacheを持たない。
