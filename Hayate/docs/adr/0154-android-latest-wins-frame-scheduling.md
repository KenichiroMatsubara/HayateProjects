---
status: accepted
---

# Android Latest-Wins Frame Scheduling

AndroidのPlatform FrontからRaster Handoffまでを一つのlatest-wins single-flight policyとして扱う。16ms timer polling、unbounded/FIFO frame queue、UI threadのraster待ちは採らない。Platform Frontはpending Choreographer callbackを最大一つだけ所有し、複数の`request_redraw`を次のvsyncへ集約する。Choreographerの`frameTimeNanos`をframe timestampの正本とし、一回のcallbackでApp Hostを最大一回だけcommitする。次vsyncを予約するのはApp Hostがpending visual workを返した場合に限り、idle時はcallback、timer、renderを発生させない。

UI threadはRaster Threadを待たず、handoffは処理中frame一件と置換可能なpending frame一件を上限とする。新しいCommitted FrameはpendingのScene SnapshotとLayer Topologyを最新へ置換する一方、置換されるframeのcontent/chrome dirty、topology変更、Layer Presentationの未反映workをunionして失わない。これにより古いanimation frameを後から再生せず、処理可能な最新stateを表示する。

resize、surface lost、surface rebuildなどのlifecycle commandはframeとcoalesceせず、前後の相対順序を維持する。lifecycle commandを跨いでpending frameを置換してはならない。raster遅延が継続する場合もqueueを増やさず、観測値をRender Scaleへ渡して段階的劣化を判断させる。

## Considered Options

- 16ms timer pollingはvsyncと位相が揃わず、idle wakeも残るため不採用。
- FIFOまたはunbounded queueは古いframeを遅れて表示し、入力結果が数秒後に追いつくため不採用。
- UI threadをraster completionまでblockする案は入力・layoutまで停止させるため不採用。
- 最新frameへ単純置換してdirtyを捨てる案は、coalesced frameだけが持つcache修復を失うため不採用。

## Consequences

- wakeが一vsync内に何回来てもApp Host pumpは一回となる。
- Raster Threadが詰まってもpending frame memoryは一件を上限とする。
- coalescing testは最新snapshot、dirty union、lifecycle順序、idle停止をobservable outcomeとして固定する。
- Platform Frontはscheduleを所有するが、継続要否はApp Host、renderer資源はRaster Threadが所有する。
