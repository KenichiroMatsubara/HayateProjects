# @tsubame/example-react-todo

`@torimi/tsubame-react`（react-reconciler ベースの Adapter）の最小デモ。React Fiber の更新を
Tsubame Renderer Protocol 経由で **DOM Renderer** に流し、素の TODO を描画する。

`examples/todo`（SolidJS 版・Canvas/DOM 両対応の大きめのデモ）と対になる、React 経路の
最小サンプル。ブラウザ向けの既定経路は DOM Renderer のみだが、**Torimi**（FW 非依存
dev-client）向けには Canvas 経路の App Bundle も出力する（下記）。

## Torimi — FW 非依存の実証（#531 / ADR-0001）

Torimi の中核主張は「Viewer 一本で全 JS フレームワークが動く」こと。`examples/todo`
（solid）と**同じ FW 非依存ホスト**（`host.html` / `@torimi/host-web`）に、この React 版の
App Bundle を流し込んで描画する。ホスト側に react 固有のコードは一切無く、react と
`@tsubame/renderer-canvas` は**バンドルが持ち込む**（`src/main.bundle.tsx` — `@torimi/bundle` の
`registerTorimiApp` を呼ぶ、Native/Web 全ターゲット共通の 1 エントリ, #767）。

- `src/host-boot.ts` は solid 版と **byte 同一**（＝文字通り同じホスト）。FW 分岐が無いことは
  `src/torimi-host-fw-agnostic.test.ts` が守る。
- バンドルは `__torimiMount` / `__torimiProtocolVersion` を露出するだけで、ホストは
  中身の react を解さない（`src/main.bundle.test.ts`）。

```sh
pnpm torimi:web:build   # dist-torimi/bundle.js（単一 IIFE）を出力
pnpm test              # ユニット（FW 非依存ホスト + mount 契約）
pnpm test:e2e          # 実 Chromium で host.html に react バンドルを流し込み描画を検証
```

## デプロイ（GitHub Pages）

`main` への push で `.github/workflows/deploy-pages.yml` が走り、Solid 版は Pages の
ルートに、本 React 版は **`/react/` サブパス**に同梱されて公開される。

- Solid 版: https://kenichiromatsubara.github.io/HayateProjects/
- React 版: https://kenichiromatsubara.github.io/HayateProjects/react/

## 動かす

ワークスペースの Tsubame パッケージをビルドしてから dev サーバを起動する
（`@torimi/tsubame-react` などは `dist` を参照するため）。

```sh
# リポジトリルートで一度だけ
pnpm install
pnpm build:tsubame

# このディレクトリで
pnpm dev        # 開発サーバ
pnpm build      # 本番ビルド（dist/）
pnpm typecheck  # 型チェック
```

## できること

- seed タスクの表示と「残り N / 全 M 件」の集計
- `text-input` での新規タスク追加（追加ボタン / Enter）
- チェックボックスまたは行クリックで完了トグル（`line-through`）
- `×` で個別削除、「完了済みを削除」で一括削除

## 仕組み

- JSX は React 標準の automatic runtime で変換し、`jsxImportSource` を
  `@torimi/tsubame-react` に向けるだけ（`vite.config.ts` / `tsconfig.json`、ADR-0010）。
- `<view>` / `<text>` / `<button>` / `<text-input>` / `<scroll-view>` は
  Tsubame の Element 語彙。スタイルは `HayateCssStyle`。
- `renderTsubame(<App />, new DomRenderer({ container }))` で DOM に mount する。
