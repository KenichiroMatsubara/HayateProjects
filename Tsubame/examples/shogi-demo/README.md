# @tsubame/example-shogi-demo

`@torimi/tsubame-react` から Hayate の入力と即時2D描画を使う、スマホ向けの将棋GUI。
solid / react の各デモと**同じ FW 非依存ホスト**で動く 3 つ目の App Bundle。

## 何を実証するデモか

Sketch（`react-demo`）より遥かに状態と描画が重いアプリで、
「FW もレンダラもバンドルが持ち込み、ホストは無改造」（ADR-0001）が保つことを示す。

## ルールは `tsshogi`（MIT）に委ねる

合法手・二歩・打ち歩詰め・行き所のない駒・千日手・連続王手の千日手の判定は
[`tsshogi`](https://github.com/sunfish-shogi/tsshogi) が正本。`src/rules/` はその上の
薄いラッパ（対局の状態機械と、描画・当たり判定が要る形への射影）だけを持つ。
禁じ手判定を自前で持たないのは二重管理を避けるため。

## 描画の分担（draw v1 の制約から決まる）

`DrawCanvas`（draw v1・ADR-0141）には**テキスト描画命令が無い**。そこで:

- **`draw` painter**（`src/paint/`）: 木地・罫線・星・ハイライト・合法手ドット・**駒の五角形**
- **要素ツリー**（`src/ui/`）: 駒の**漢字**と操作 UI

`InteractionEvent.x/y` は viewport 座標で要素ローカルに落とせないため、マスの当たり判定は
座標計算ではなく **81 個のセル要素**で取る。描画順は background → border → draw → children
なので、painter の五角形は自動的に漢字の下に来る。

### 後手の駒の向き

五角形は `DrawCanvas.rotate` で 180° 回せるが、**漢字は回せない**（draw に文字命令が無く、
`HayateCssStyle` にも `transform` が無い）。そこで後手は「五角形は下向き・漢字は正立で別色」
で表す。draw にテキストが生えたら漢字も同じ回転に乗せられる（ADR-0141 は
テキストを「封印ではなく後回し」と明記している）。

## 起動

```sh
pnpm dev          # ?renderer=dom / tiny-skia / vello で切替
pnpm build
pnpm typecheck
pnpm test
pnpm test:e2e
```

## Torimi

react-demo / solid-demo と同じホストに、React・Tsubame Adapter・Hayate Renderer を含む
App Bundle を流し込む。`src/main.bundle.tsx` は `registerTorimiApp` を呼ぶ Native/Web 共通
entry で、`src/host-boot.ts` は react-demo と**バイト一致**（FW 非依存の証明そのもの）。

```sh
pnpm torimi:native:build
pnpm torimi:web:build
```

## e2e の観測点

Hayate Renderer 経路では盤が canvas に閉じて DOM から見えないので、局面の主張は
`?debug=1` で生える `window.__shogiDebug`（`sfen()` / `lastMove()` / `ply()`）越しに行う。
