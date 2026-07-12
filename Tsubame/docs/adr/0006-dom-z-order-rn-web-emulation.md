# DOM Renderer の Z-Order は RN Web 現行方式で Hayate セマンティクスをエミュレートする

> **用語更新（ADR-0011・2026-06-27）**: 本 ADR の "Canvas Renderer" / `CanvasRenderer` / `@tsubame/renderer-canvas` は **Hayate Renderer** / `HayateRenderer` / `@torimi/tsubame-renderer-hayate` に改名された。本文は決定当時の記録として原文のまま。
>
> **一部 supersede（ADR-0013）**: 下記「静的コード」節（`DomStylePatch` による IDE 診断）は撤回された。adapter が renderer 非依存であるため型分岐の利用箇所が成立しなかったため。「動的値」節（runtime `console.warn`）は変更なく有効。

_origin: Hayate ADR-0021, react-native-web `View` base styles_

Tsubame の DOM Renderer はブラウザ CSS をそのまま正本にせず、Hayate / Canvas Renderer と同じ React Native 方式の Z-Order セマンティクスを DOM 上で再現する。実装は react-native-web の現行 master が採用する方式（全 element に `position: relative` + デフォルト `zIndex: 0`）を踏襲する。ブラウザ CSS との完全一致は目標にしない。

## Z-Order セマンティクス（Hayate 正本）

Hayate ADR-0021 に従い、開発者向けの契約は次のとおり。

1. **第一原則は document order（後勝ち）** — 同一 parent 内で `zIndex` 未指定（デフォルト 0）の兄弟は、後から追加されたものが上に描画される
2. **`zIndex` は兄弟内の上書き** — 数値が高い兄弟が前景。親の兄弟より前面に出すことはできない
3. **親をまたぐ重なりは root 直下配置** — モーダル・tooltip 等は Portal 相当で root に置く

Canvas Renderer は Hayate コアの `scene_build` がこの契約を保証する。DOM Renderer は RN Web 方式で同等の境界を作る。

## DOM Renderer の実装方針

- **全 Element kind**（`view` / `text` / `image` / `button` / `text-input` / `scroll-view`）のベーススタイルに `position: relative` と `zIndex: 0` を付与する（`dom-elements.ts`）
- 開発者が `setStyle` で指定した `zIndex` はベーススタイルを上書きする
- RN Web の PR #2808（デフォルト `zIndex: 'auto'`）は採用しない。Hayate の「兄弟内のみ」境界を DOM で再現するには現行方式が最も直接的である

## 既知の差分と警告

Hayate は `opacity` 等による暗黙 stacking context を持たないが、ブラウザ CSS は持つ。この差分は DOM パスでのみ発生しうる。

- **拒否はしない** — `opacity` 等の表現力は維持する
- **警告対象は拡張可能な registry** — 現行は `opacity`、Element スタイルに追加された `transform` を含む
- **静的コード** — `DomStylePatch` の TypeScript `@deprecated` で IDE ファイル診断とする（ビルド失敗なし、`tsubame lint` CLI は設けない）
- **動的値** — `NODE_ENV !== 'production'` のときのみ runtime `console.warn`（同一 element + property につきセッション 1 回）
- **将来** — ESLint によるファイル警告は理想だが初期スコープ外

## Considered Options

- **ブラウザ CSS そのまま（却下）** — `zIndex` を inline style に渡すだけでは `position: static` で無視されたり、`opacity` / `transform` で stacking context が漏れ、Canvas パスと意味がズレる
- **RN Web 提案方式 `zIndex: 'auto'`（却下）** — ネスト時の z-index 地獄は減るが、兄弟内隔離が弱く Hayate ADR-0021 との対応が崩れやすい
- **`opacity` / `transform` を DOM で拒否（却下）** — Z-Order 保証は強いが表現力が落ちる
- **RN Web 現行方式 + TS 警告（採用）** — 十年の実績があり、Hayate 正本との対応が明確。完全一致は諦めるが「そこそこいい感じ」で統一セマンティクスを守れる

## Consequences

- `packages/renderer-dom/src/dom-elements.ts` に全 kind 共通のベース stacking スタイルを追加する
- `packages/renderer-dom` が `DomStylePatch` を export し、`opacity` 等に `@deprecated` を付与する
- `DomRenderer.setStyle` が registry に基づき dev 警告を出す
- adapter は DOM ターゲット時に `DomStylePatch` を使うよう型を分岐できる（任意・段階的）
- Hayate HTML Mode とは別経路である点は変わらない（DOM Renderer は Hayate を使わない）
