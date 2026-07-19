# @tsubame/example-react-todo

`@torimi/tsubame-react`からHayate独自の入力・即時2D描画を使う、スマホ向けの簡易スケッチデモ。
パッケージ名と配信URLの`react-todo`は互換のため維持している。

## MVP

- 1本指／mouse／penのpointer down・move・upから連続ストロークを描く
- Undo 1段ずつ、Clear、細／太ブラシ切替
- point列と進行中ストロークは`SketchDocument`が所有し、React stateには載せない
- pointer moveはHayate coreで1px未満をcoalesceし、押下要素へのcapture付きdeliveryとしてJSへ渡す
- Androidの座標はnative hostでsafe-area補正・論理px化済み

レイヤー、画像保存、ブラシエンジン、pressure、マルチタッチは初期MVPの対象外。

## Torimi

Solid版と同じframework非依存ホストに、React・Tsubame Adapter・Hayate Rendererを含むApp Bundleを
流し込む。`src/main.bundle.tsx`は`registerTorimiApp`を呼ぶNative/Web共通entryで、
`src/host-boot.ts`にReact固有コードはない。

```sh
pnpm torimi:native:build
pnpm torimi:web:build
pnpm test
pnpm test:e2e
```

本番Demo Endpointへの配信はAndroidホストと同じtagからlockstep workflowで行う。
手動デプロイは明示依頼がある場合だけ行う。詳細は`Torimi/demo-endpoint/README.md`を正本とする。

## Web開発

```sh
pnpm dev
pnpm build
pnpm typecheck
```

`?renderer=dom`でDOM Renderer、`?renderer=tiny-skia`等でHayate Rendererを選べる。
