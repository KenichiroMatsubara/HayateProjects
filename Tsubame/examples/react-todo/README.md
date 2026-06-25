# @tsubame/example-react-todo

`@tsubame/react`（react-reconciler ベースの Adapter）の最小デモ。React Fiber の更新を
Tsubame Renderer Protocol 経由で **DOM Renderer** に流し、素の TODO を描画する。

`examples/todo`（SolidJS 版・Canvas/DOM 両対応の大きめのデモ）と対になる、React 経路の
最小サンプル。Hayate（WASM/Canvas）は使わず DOM Renderer のみを使う。

## デプロイ（GitHub Pages）

`main` への push で `.github/workflows/deploy-pages.yml` が走り、Solid 版は Pages の
ルートに、本 React 版は **`/react/` サブパス**に同梱されて公開される。

- Solid 版: https://kenichiromatsubara.github.io/HayateProjects/
- React 版: https://kenichiromatsubara.github.io/HayateProjects/react/

## 動かす

ワークスペースの Tsubame パッケージをビルドしてから dev サーバを起動する
（`@tsubame/react` などは `dist` を参照するため）。

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
  `@tsubame/react` に向けるだけ（`vite.config.ts` / `tsconfig.json`、ADR-0010）。
- `<view>` / `<text>` / `<button>` / `<text-input>` / `<scroll-view>` は
  Tsubame の Element 語彙。スタイルは `HayateCssStyle`。
- `renderTsubame(<App />, new DomRenderer({ container }))` で DOM に mount する。
