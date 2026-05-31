# HTML Mode のレンダリングをブラウザ CSS レイアウト方式に切り替える

ADR-0016 で確定した HTML Mode の描画アプローチ（Taffy → absolutely-positioned div）を廃止し、
ブラウザの CSS エンジンにレイアウトを委ねる方式に切り替える。

## Context

ADR-0016 は HTML Mode の描画を以下のパイプラインで定義した：

```
Element Layer → Taffy（レイアウト計算）→ Raw Layer（絶対座標）
  → absolutely-positioned div にマッピング
```

各要素に `position: absolute; left: Xpx; top: Ypx; width: Wpx; height: Hpx` を直接設定する方式。

このアプローチは要素数 N に対して DOM 書き込みが **4N** にスケールする。

| 要素数 | DOM 書き込み数 | 実用性 |
|---|---|---|
| 100 要素 | 400回/layout pass | 許容範囲 |
| 500 要素 | 2000回/layout pass | 重くなり始める |
| 1000 要素（IDE・Infinite Canvas 等） | 4000回/layout pass | 実用上きつい |

Hayate が狙うユースケース（IDE のミニマップ・AI チャット UI・Infinite Canvas）は
大規模 UI が前提であり、現行方式は設計目標と合わない。

## Decision

HTML Mode の描画方式を以下に切り替える：

**変更後**:
```
Element Layer → Hayate CSS プロパティをブラウザ CSS プロパティに直接マッピング
              → ブラウザの CSS エンジンがレイアウトを計算
```

- **Taffy は HTML Mode のパイプラインから除外する**。Taffy は Canvas Mode とネイティブのみで使用する
- Hayate CSS の各プロパティを対応するブラウザ CSS プロパティに 1:1 マッピングする
  （`display: flex` → `display: flex`、`gap` → `gap` 等）
- 変更したプロパティのみ書き込む。ブラウザのインクリメンタル reflow に委ねる
- `ElementId` は引き続き DOM 要素との対応キーとして使用する（ADR-0016 §5.6 の安定 key 設計は維持）

## Known Limitations

ブラウザ CSS の意味論と Hayate の意味論が一致しない領域は、
既知制限としてドキュメントに記載し、実装上の回避は行わない。

| Hayate の意味論（ADR-0021） | ブラウザ CSS の意味論 |
|---|---|
| `transform` はスタッキングコンテキストを作らない | `transform` はスタッキングコンテキストを作る |
| `opacity < 1` はスタッキングコンテキストを作らない | `opacity < 1` はスタッキングコンテキストを作る |
| `z-index` のスタッキングスコープは同一 parent 内 | CSS はスタッキングコンテキストをネストする |

HTML Mode は「開発時の UI 確認」と「非 Chromium ブラウザでの動作確認」が主用途であり、
Canvas Mode / Native との**完全一致より実用的な精度で十分**という判断。

## Considered Options

- **Taffy → absolutely-positioned div（ADR-0016 元方式、廃止）**:
  Canvas Mode と同一パイプラインを経由するため理論上レイアウト一致が保証されるが、
  4N DOM 書き込みがスケールしない。IDE 相当の UI では実用上きつい。

- **ブラウザ CSS レイアウト（採用）**:
  変更差分のみの CSS 書き込みで済み、ブラウザの C++ 実装による高速インクリメンタル reflow を活用できる。
  500 要素超で Taffy 方式より明確に速い。意味論的なズレは既知制限として文書化する。

## Sell Line との整合

本 ADR の変更後、HTML Mode と Canvas Mode の役割は以下のように明確に分かれる：

| モード | 条件 | 特性 |
|---|---|---|
| Canvas Mode | WebGPU + EditContext 対応（現在 Chromium） | Taffy + Vello + バンドルフォント → Native とピクセル完全一致 |
| HTML Mode | 上記以外（Firefox・Safari 等） | ブラウザ CSS レイアウト → 実用的な UI 確認に十分 |

主要 DX：**Chrome で開発すれば Native と完全一致した状態で UI を確認できる**。
TypeScript で作れば HTML Mode も十分実用的な開発体験を提供する。

## Consequences

- `crates/adapters/web`: `HayateElementHtmlRenderer` をブラウザ CSS マッピング方式に書き直す（Issue #32）
- `hayate-spec.md` §4.1: HTML Mode の説明を更新（本 ADR に追随）
- `ADR-0016`: HTML Mode の描画アプローチ記述を本 ADR が上書きする
- `CONTEXT.md`: HTML Mode 定義は更新済み
- ADR-0004（Taffy）: HTML Mode での使用が除外され、Canvas Mode・Native 専用になる
