---
status: accepted
---

# Browser Host adapter を専用 App 層 package に集約する

Solid・React・Draw Gallery の browser entry は、DOM / Hayate target 選択、dev tuning の取得、`createHayateWebHost`、`HayateRenderer::start`、renderer / Worker teardown をそれぞれ組み立てている。ADR-0012 が抽出条件とした二つ目を越えて三つの同型 adapter が存在し、React だけ Worker detach が欠けるなど lifecycle の非対称も既に生じている。

これらを新しい App 層 package `@torimi/tsubame-browser-host` に集約する。この package は `@torimi/tsubame-app` の `Host` port を実装し、`@torimi/tsubame-renderer-dom`、`@torimi/tsubame-renderer-hayate`、`@torimi/hayate-host` への browser 固有依存を一箇所に閉じる。各 framework entry は DOM container / canvas 等の browser surface と framework 固有 `TsubameMount` だけを供給する。

`@torimi/tsubame-app` は引き続き `@torimi/tsubame-renderer-protocol` だけに依存する純粋な Composition Root とし、具体 renderer や Hayate runtime を import しない。Torimi の `@torimi/host-web` は dev-server fetch / protocol handshake / reload を所有する別 bounded context であり、この Tsubame browser composition adapter と統合しない。

ADR-0012 の `Host.createRenderer() + stop?()` は `Host.start() -> HostSession | Promise<HostSession>` に置き換える。`HostSession` は `renderer: IRenderer` と必須かつ冪等な `dispose()` を一つの lifetime value として返す。`runTsubameApp` は通常時に mount dispose の後で session を破棄し、非同期 `start` の完了前に dispose された場合は、完了した session を mount せず直ちに破棄する。これにより renderer / Worker が作られる前に別の optional `stop` を呼んでしまう late-resolve leak を構造的に防ぐ。DOM Host も no-op `dispose` を持ち、lifecycle contract に例外を作らない。

DOM / Hayate target selection も Browser Host が所有し、`@torimi/tsubame-app` と framework entry から `shouldUseDomRenderer` / `useDomRenderer` を除く。Browser Host は DOM container と canvas の両 surface を受け取り、`?renderer=dom` または EditContext 非対応なら DOM、それ以外なら Hayate を選ぶ。`dom` 以外の query 値は Browser Host で backend enum として解釈せず、Hayate 内部の Scene Renderer 選択は Hayate Host / Rust Render Host に残す。production は `window.location.search` と `EditContext in window` を読み、純粋テストでは同じ二値を environment seam から注入できる。

`runTsubameApp` の戻り値は単なる `Dispose` から `AppHandle { settled, dispose }` に置き換える。`settled` は reject しない `Promise<AppStartResult>` で、結果を `{ status: 'mounted' } | { status: 'failed', error } | { status: 'disposed' }` として返す。Host start または framework mount の失敗を `console.error` だけに畳まず caller が UI / log / test へ接続できるようにし、start 完了前の dispose は直ちに `disposed` として settle する。遅れて完成した `HostSession` の破棄は結果 settlement 後も必ず遂行する。

Browser Host の `start` は resource transaction とする。開始前に DOM container / canvas の visibility を記録し、選択 surface を表示して正しい寸法を確立してから、tuning load → Hayate Web Host / Worker 作成 → `HayateRenderer` 作成 / start の順に進む。各段階で得た cleanup を resource stack に登録し、途中失敗・late dispose・通常 `HostSession.dispose` のすべてで `renderer.stop → webHost.detach → visibility restore` の逆順に実行する。一つの cleanup が失敗しても残りを必ず続け、dispose 自体は冪等とする。DOM 経路も mount dispose 後に元の visibility を復元する。

開発用 tuning は暗黙に network fetch せず、Browser Host option の明示的な `TuningSource = none | inline(json) | optional-url(url)` で受ける。既定は `none`。`optional-url` の 404 / network failure はコンパイル済み既定で続行し、取得成功時だけ生 JSON を `createHayateWebHost` へ渡す。DOM target では tuning source を評価しない。fetch と best-effort 規則は package 内の一実装に集め、テストは inline source または fetch seam を注入する。

`globalThis.__hayateHost` は Torimi native wire が注入する `RawHayate` 専用名として予約し、browser app は WebHost や raw を代入しない。Browser Host は選択 target と `pipelineObservation()` だけを持つ読み取り専用 `BrowserHostInspection` を返せるが、raw / renderer / Worker transport は公開しない。demo / E2E が inspection を global に載せる場合も caller の明示的な dev glue とし、専用名 `__tsubameBrowserHostInspection` を使って session dispose 時に解除する。package 自身は global を自動生成しない。

## Considered Options

- **browser 実装を `@torimi/tsubame-app` に追加する** — 利用側 package は増えないが、ADR-0012 の runtime-blind な Composition Root に DOM Renderer / Hayate Renderer / Hayate Host 依存が流入するため採用しない。
- **各 example の inline Host を維持する** — 三実装で lifecycle 差が発生済みで、framework 追加ごとに boot / teardown が複製されるため採用しない。
- **Torimi `@torimi/host-web` に統合する** — Torimi host は dev-server と reload の product semantics を持つ。通常の Tsubame browser app composition を Torimi 開発ホストへ依存させるため採用しない。
- **`createRenderer() + stop?()` を維持し、Browser Host 内部だけで late resolve を補償する** — renderer と teardown の lifetime が port 上で分離したままで、今後の Host 実装も同じ補償を再実装できてしまう。利用箇所が少ない現時点で `HostSession` に置き換える。
- **framework entry が `shouldUseDomRenderer` を呼び、選択済み target を Browser Host へ渡す** — target policy が各 entry に残り、Vue 等を増やすたびに分岐が再出現するため採用しない。
- **Browser Host が `dom | hayate | vello | tiny-skia ...` を解釈する** — Tsubame の browser composition に Scene Renderer vocabulary と選択順が漏れ、Hayate Render Host と二重正本になるため採用しない。
- **boot failure を Composition Root 内で `console.error` するだけにする** — caller と統合テストが mount 成否を型で観測できず、専用 error UI に接続できないため採用しない。
- **`AppHandle.ready` を reject させる** — caller が handle を観測しない一般 entry で unhandled rejection を作る。開始結果を値として resolve する `settled` を選ぶ。
- **成功時だけ teardown closure を組み立てる** — Worker 作成後・renderer start 前などの部分失敗で登録済み資源が孤児化するため採用しない。start と teardown を一つの resource transaction にする。
- **既定で `document.baseURI/tuning.jsonc` を fetch する** — package 利用者の production 起動に暗黙の network I/O と 404 を追加するため採用しない。利用する app が source を明示する。
- **各 framework entry が tuning を fetch して JSON 文字列を渡す** — fetch failure の扱いと順序が再び entry ごとに複製されるため採用しない。entry は declarative な source だけを指定する。
- **Solid browser demo が `__hayateHost` / `__hayateRaw` に debug object を公開し続ける** — Torimi native wire と同じ global 名へ異なる型を入れ、raw escape hatch も恒久 API 化するため採用しない。narrow inspection seam と専用 dev global に分離する。

## Consequences

- browser の target selection、boot、start、完全 teardown は一つの test surface になる。
- framework entry は Solid / React / 将来 Vue の mount 呼び形だけを残す。
- `@torimi/tsubame-browser-host` は browser 専用であり、native / bundle Host を一般化して飲み込まない。
- ADR-0012 の段階的抽出条件は本 ADR により満たされたものとする。
- `runTsubameApp` の dispose は pending `Host.start()` を同期 cancel できなくても、late-resolved `HostSession` を必ず破棄する。
- framework entry は target boolean を持たず、選択結果に応じた surface visibility と boot も Browser Host session が管理する。
- Torimi や example は `AppHandle.settled` を必要に応じて error panel / data attribute / test assertion へ接続できる。
- Browser Host の unit test は各 start 段階で fault injection し、登録済み cleanup が逆順で全実行され visibility が復元されることを検証する。
- tuning test は DOM target では source が未評価、Hayate target の optional URL failure では boot 継続、成功時は最初の app frame 前に適用されることを固定する。
- browser E2E は `BrowserHostInspection` を使い、`RawHayate` を直接操作しない。native wire test は `__hayateHost` が `RawHayate` 以外を受けないことを固定する。
