# 合成ルートを deep module 化し、`Host` ポートと唯一の FW 固有 `mount` seam に畳む

**Status: accepted**

**Date: 2026-06-27**

## Context

`tsubame-react` は「DOM でしか描画されない」と観測された。原因は adapter の欠陥ではない — `renderTsubame(node, target: IRenderer)` はどの `IRenderer` でも描け、`main.miharashi.tsx` は現に `HayateRenderer`（旧 CanvasRenderer）で Hayate に描いている。欠けているのは **target 選択・host 配線・renderer 取得・mount を束ねる合成ルートが module になっていない**ことだった。

現状の非対称:

| | DOM/Canvas 出し分け | Canvas(Hayate) 経路 |
|---|---|---|
| **solid example** | `main.tsx` 内で `detectMode` 分岐（`mountCanvasApp` / `detect-mode.ts` が helper） | 同 `main.tsx` で `vite dev` から直接 |
| **react example** | `main.tsx` は `DomRenderer` 固定 | `main.miharashi.tsx` に隔離（Miharashi host 経由のみ） |

`mountCanvasApp` / `detectMode` は**すでに FW 非依存**だが solid example の `src/` に閉じており、react から再利用できない。結果、FW を足すたびに mode 検出・host 配線・renderer 選択を各 example で書き直す構造になっていた。ADR-0002 は「Adapter はどの Renderer を使うか意識しない」を既に明言しており、この非対称は ADR の意図に反した実装漏れである。

**削除テスト**: `compose.ts` + `detect-mode.ts` + `main.tsx` の分岐を消すと、mode 検出・host 配線・renderer 選択の複雑さが FW ごとの entry に N 個再出現する → 複雑さは消えず複製される。seam を一つに集める価値がある。

## Decision

合成ルートを deep module に昇格し、3 つの軸を分離する。

```ts
// @tsubame/app —— @tsubame/renderer-protocol だけに依存（Hayate ランタイム盲目）
interface Host {
  createRenderer(): IRenderer | Promise<IRenderer>; // DOM か Hayate かは Host が決める
  stop?(): void;                                     // frame-clock / WASM teardown
}
type TsubameMount = (renderer: IRenderer) => Dispose; // 唯一の FW 固有 seam
function runTsubameApp(host: Host, mount: TsubameMount): Dispose;
```

1. **合成ルート `runTsubameApp` は純粋 orchestrator**。`@tsubame/app` に置き、`@tsubame/renderer-protocol` だけに依存する。`renderer-dom` / `renderer-hayate` も `@hayate/host` も import しない。具体 renderer 名も platform も知らず、見るのは `IRenderer` だけ。これにより CONTEXT.md の依存境界「Tsubame → Hayate は Contract のみ／host bootstrap を Tsubame renderer パッケージに置かない」を破らない。
2. **`Host` ポートが renderer を産む**。`createRenderer()` の中で `DomRenderer` か `HayateRenderer` を `new` し（Hayate なら `start()` まで）、`IRenderer` を返す。**「DOM か Hayate か」「web か native か bundle か」の分岐は Host 実装に局在する**。platform 増殖（web-vello / web-tinyskia / Android / 将来 iOS・Desktop）はすべて `Host` を 1 つ足す仕事に縮み、renderer は Dom / Hayate の二つで固定。
3. **`mount` が唯一の FW 固有 seam**。`(renderer: IRenderer) => Dispose`。各 `Tsubame Adapter`（solid / react / vue）はこの 1 関数を供給するだけ。`renderTsubame` の呼び形の差（solid=`() => JSX`、react=`ReactNode`）は reactivity に内在する**正当な非対称**で、この seam の内側に閉じ込める（統一しない）。FW を足すコストはこの 1 関数。
4. **web の DOM/Canvas 判定 `detectMode` は依存ゼロの純粋関数**として orchestrator の外に置く。`detected` を App が読める（example は `<App detected>` で表示に使う）よう、entry が `detectMode` を呼んで結果を Host 構築と mount クロージャの両方へ渡す。
5. **Host adapter の置き場所は段階的**。`@tsubame/app`（純粋）にも `@hayate/host`（Hayate→Tsubame は永久に依存なし）にも置けないため、host adapter は App 階層のグルー。各 5 行程度なので**当面は各 example に inline**。solid-todo と react-todo で `webHost` が同一になった時点＝2 つ目の adapter＝そこで初めて中立 App 階層パッケージへ抽出する（「1 adapter は仮の seam、2 adapter は本物の seam」）。

## 逆向き経路（Miharashi / native）の収まり

bundle / native は host が「押し込まれる」向きだが seam は割れない:

- **bundle**: `__miharashiMount = (webHost) => runTsubameApp(canvasHostFrom(webHost), mount)` — 押し込まれた raw+clock を `Host` に包んで同じ呼び。`__miharashiMount` / protocol version の wire 契約は不変（ADR-0001 / Miharashi ADR-0001）。
- **native**: `createHayateNativeHost(raw)` を `Host` に包み `runTsubameApp`。`globalThis.__tsubame.pumpFrame` は entry が握る host 参照から、`stop` は `runTsubameApp` の戻り `Dispose` から出す。

## Considered Options

- **単一 `@tsubame/app` が `@hayate/host` を直接 import**: 利用は最も簡単だが `@tsubame/*` パッケージが Hayate ランタイムに依存し CONTEXT 境界・ADR-0004 を破る。→ 却下。純粋 orchestrator + Host adapter 別置きを採る。
- **orchestrator が renderer を構築**（HostAdapter は primitive のみ供給）: orchestrator が `renderer-dom` / `renderer-hayate` を import し DOM/Canvas 分岐を背負う。純度が落ち「なぜ Tsubame が具体 renderer を知る？」が再発。→ 却下。Host が renderer を産む。
- **合成ルートを共有せず各 example に開コードのまま**: churn 最小だが FW 追加で host 配線を書き直す問題が残り、削除テストに照らして複雑さが複製される。→ 却下。
- **host adapter を最初から共有パッケージに抽出**: 1 adapter しか無い段階で seam を作るのは投機的。→ 却下。2 つ目が出てから抽出。

## Consequences

- 「`tsubame-react` が DOM でしか描かれない」は構造的に起こり得なくなる（target は `Host` が決め、FW は mount のみ）。
- FW 追加コストが `TsubameMount` 1 関数に縮む。platform 追加コストが `Host` 1 実装に縮む。
- `examples/todo`（solid）の `compose.ts` / `detect-mode.ts` / `main.tsx` 分岐と、`examples/react-todo` の DOM 固定は、`@tsubame/app` の `runTsubameApp` ＋ inline Host adapter ＋ FW ごとの mount へ移行する（実装は別 PR）。
- テスト境界が `runTsubameApp` の interface に集約: fake `Host` ＋ fake `mount` で「createRenderer→mount→dispose が stop を合成する」を assert できる。`detect-mode.test.ts` は純粋関数テストとして存続、`miharashi-host-fw-agnostic` の「同一 host が両 FW を描く」性質は両者が `runTsubameApp` を呼ぶことで構造的に成立する。
- ADR-0011（`HayateRenderer` 改名）を前提とする — host adapter が `new HayateRenderer(...)` する。
- 関連: ADR-0002（Adapter は renderer を意識しない・本 ADR が実装で回収）、ADR-0004（adapter/App が runtime を持つ）、ADR-0010（react mount の構造ゼロ host config）、ADR-0001 / Miharashi ADR-0001（host 契約不変）。
