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

## 描画の分担

`draw` にテキスト命令が生えた（#732 / ADR-0141 が「封印ではなく後回し」と書いていた
ものを行使した）ので、**駒は絵として完結する**。

- **`draw` painter**（`src/paint/`）: 木地・罫線・星・ハイライト・合法手ドット・
  **駒（五角形と漢字の両方）**・駒台
- **要素ツリー**（`src/ui/`）: **当たり判定**と操作 UI だけ

`InteractionEvent.x/y` は viewport 座標で要素ローカルに落とせないため、マスの当たり判定は
座標計算ではなく **81 個のセル要素**で取る。ただしそのセルは子を持たない透明な箱で、
絵は 1 枚の display list に載る。

### 駒は 1 つの描画単位

`src/paint/piece.ts` の `paintPiece(canvas, rect, piece, flipped)` が**駒を描く唯一の場所**で、
盤（`board-painter.ts`）も駒台（`hand-painter.ts`）もこれを呼ぶ。

先後の別は `flipped` **1 つ**で表す。中身は `save()` → 駒の中心へ `translate` →
`flipped` なら `rotate(π)` → 五角形と漢字を**同じ変換の下で**描く → `restore()` なので、
「図形だけ回って文字は正立」という破れが構造的に起こり得ない。地色も先後で変えない
（紙の将棋と同じで、向きが先後を表す）。

## 日本語の字

`NotoSansJP.ttf` は `include_bytes!` で Hayate core にバンドルされ、既定 family かつ
sans-serif の総称として登録済み。**漢字は初回フレームからネットワーク無しで出る。**
CDN（`Hayate/crates/platform/web/fonts.json`）から実行時に取るのは韓国語・簡繁体・
記号・絵文字で、将棋盤には要らない。

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
