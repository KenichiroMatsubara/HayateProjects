# tsubame-react は react-reconciler HostConfig（write-only・構造ゼロ instance）とし、リスナのライフサイクルを backend に寄せる

**Status: accepted**

**Date: 2026-06-25**

## Context

CONTEXT.md は `tsubame-react` を Tsubame Adapter（`tsubame-solid` / `tsubame-vue` / `tsubame-react` の総称）の一員として既に語彙化しており、ADR-0062 は「VDOM を持つ tsubame-react は構造を読み返さないため shadow tree を必要としない」と先に decide していた。本 ADR はその想定を実装方針として確定する。

React には `solid-js/universal` のような「コンパイル時に JSX の import 先を差し替える」機構が無い。React を非 DOM backend に向ける正攻法は `react-reconciler`（react-dom / React Native / react-three-fiber / Ink を支える非公開・unstable API）の HostConfig を書くことである。これは Solid と構造的に異なる:

| | reconcile 時にホスト構造を読むか | ホスト要件 |
|---|---|---|
| **solid-js/universal** | 同期で読む（VDOM なし） | 同期で読める retained ツリー（= shadow tree）が必須 |
| **React Fiber** | 読まない（自前 Fiber tree を diff） | write-only・batch・境界越しで可 |

React の Fiber tree が Solid の shadow tree の役目（構造の記帳）を内部で果たすため、ホスト instance に `parent` / `children` を持たせる必要はない。

唯一の論点は**リスナ解除**だった。`react-reconciler` は削除 subtree の**先頭しか** `removeChild` を呼ばないため、instance に構造が無いと adapter は子孫リスナを辿って解除できない（react-three-fiber が instance に `children` を持つのはこの dispose のため）。実装を調べた結果:

- **DOM Renderer**: `removeChild` → `forgetDomSubtree` が subtree 全体を walk して子孫リスナを掃除する（backend が所有）。
- **Canvas Renderer**: `removeChild` は先頭の remove を enqueue するのみ。JS 側リスナ map の掃除は現状 **adapter（Solid の `disposeEvents` が shadow を辿る）に依存**している。Canvas Renderer は write-only で subtree を walk できないため、ここを backend 所有にするには Hayate 側の teardown 通知（Interaction Stream の Element Document Runtime 移管。CONTEXT.md「移行対象」）が要る。

## Decision

1. **`@tsubame/react` は `IRenderer` 上に載せた `react-reconciler` の HostConfig**（mutation モード・write-only）として実装する。react-three-fiber / Ink と同型。
2. **ホスト instance は構造ゼロ**: `{ id: ElementId; kind: ElementKind }`（＋自身のリスナ差し替え用の最小情報のみ）。`parent` / `children` は持たない。subtree の構造片付けは `IRenderer.removeChild` に委ねる。
3. **リスナのライフサイクルは backend が所有する**方向に寄せる。adapter は構造を辿らない。
4. **意味論は集約しない（共有ランタイムは作らない）**。framework 固有ランタイム（solid universal / react-reconciler / 将来の vue）は各 adapter が独立して持つ（CONTEXT.md の _Avoid: shared component runtime_）。共有が必要なのは「prop/要素語彙の翻訳」だけで、重い部分（`splitHayateStyle` / `assertKnownElementProperty` / `coerceElementProperty` / `dispatchElementPropertyOp`）は既に `renderer-protocol` にある。残る `EVENT_PROP` / `REJECTED_EVENT_PROPS`（イベント語彙表）を `renderer-protocol` の adapter vocabulary（`EventKind` の隣）へ移し、`tsubame-solid` はそこからの再 export に縮める。新パッケージ（adapter-core / adapter-jsx）は作らない。
5. **active-renderer グローバルは持たない**。reconciler container 生成時に renderer を束縛し、DOM↔Canvas 切替は remount で行う（Solid のグローバルホルダーは compiled JSX が固定 import する都合のもので、react には不要）。

## スコープ分割

react を成立させる最小経路を、最終形に矛盾しない形で切り出す:

- **今回**: DOM 経路（backend が既にリスナを掃除する）で react を完成させる。instance は最初から構造ゼロで書く。
- **後続（別 issue）**: Canvas 経路のリスナ teardown を backend 所有にする（Hayate の listener teardown 通知 ＋ `addEventListener` の (id, kind) 冪等化）。これにより `tsubame-solid` の shadow node から `events` / `disposeEvents` も剥がせ、shadow tree は構造専用（`{ id, kind, parent, children }`）へ軽量化できる（ADR-0062 の試算 +70 B/node に接近）。

react パッケージは今回時点で既に最終形（構造ゼロ instance）なので、後続で Hayate 協調を足しても **react 側の書き換えは発生しない**。

## Considered Options

- **instance に `children` を持たせる（react-three-fiber 方式）**: Canvas のリスナ掃除を react パッケージ内で閉じられるが、reconcile には不要な構造ミラーをリスナ掃除のためだけに復活させ、ADR-0062 の「react は構造ミラー不要」を弱める。→ 却下。backend 所有（上記スコープ分割）を採る。
- **意味論を adapter-core に集約**: CONTEXT.md の _Avoid: shared component runtime_ に抵触する読みを生む。重い意味論は既に protocol 共有済みで、残差は 5 エントリのイベント表のみ。新パッケージは過剰。→ 却下。protocol 同居を採る。

## Consequences

- `tsubame-react` は shadow tree を持たない（ADR-0062 の帰結を実装で確定）。
- `EVENT_PROP` / `REJECTED_EVENT_PROPS` の正本は `renderer-protocol` に移り、`<view onClick>` の意味は solid / react / 将来 vue で単一ソースになる。
- Canvas 経路は当面、削除された要素の JS 側リスナエントリが残りうる（誤発火はしない。Hayate が配信を止める）。これは既知の follow-up であり、上記スコープ分割で解消する。
- 関連: ADR-0062（shadow tree）、ADR-0058（text-as-element）、ADR-0059（hover 拒否）、ADR-0071（未知 prop throw）、ADR-0081（style variants）。
