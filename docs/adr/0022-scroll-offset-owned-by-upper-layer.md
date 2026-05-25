# scroll-view のスクロールオフセットは上位層が管理し毎フレーム Hayate に渡す

## Context

`scroll-view` Element のスクロール位置（offset x, y）を誰が保持・管理するかを決定する必要があった。

1. **Hayate 保持方式**: Hayate が scroll offset を内部状態として持ち、`poll-events()` の scroll イベントを自動で積算する。上位層は scroll offset を意識しない
2. **上位層管理方式**: 上位層（Hayabusa）が `poll-events()` の scroll イベントから delta を受け取って積算し、毎フレーム `element_set_scroll_offset(id, x, y)` で Hayate に渡す。Hayate は渡された値を使って `scene_build` 時にクリップと平行移動を適用するだけ

## Decision

**上位層管理方式を採用する。**

スクロール「状態」を Hayate に持たせると、イナーシャスクロール・ページスナップ・rubber-band 等の物理演算を後から加えるたびに WIT を拡張する必要が生じる。これらは UI 体験上の演出であり、Hayate の責務（描画）ではなく Hayabusa の責務（UX ロジック）である。

上位層が offset を管理することで、スクロール物理の実装を Hayabusa 側で自由に変更できる。Hayate は「渡された offset でクリップして描く」機械であり続ける。

`position: sticky` については、Hayate が受け取った scroll offset を使って `scene_build` 内でクランプ計算を行う。sticky の挙動判定は Hayate の描画パイプライン内で閉じており、上位層は offset を渡すだけでよい。

## Considered Options

- **Hayate 保持方式（却下）**: 実装がシンプルに見えるが、イナーシャ・スナップ・rubber-band 等を実現しようとすると Hayate 内部に物理演算エンジンが必要になる。プラットフォームごとに異なるスクロールのフィール（iOS vs Android vs Desktop）を WIT 経由で制御するAPIが肥大化する。また Hayate が「フレームをまたいで状態を積算する」ループを持つことは ADR-0018 の poll モデルとの整合性を損なう
- **上位層管理方式（採用）**: Hayabusa が Signal で offset を保持し、毎フレーム `element_set_scroll_offset` で Hayate に渡す。物理演算・スナップ・rubber-band は Hayabusa のライブラリ層で実装できる。Hayate の WIT は `element_set_scroll_offset(id: element-id, x: f32, y: f32)` 1 エントリだけ追加すれば足りる

## Consequences

- `element_set_scroll_offset(id: element-id, x: f32, y: f32)` を Element Layer WIT に追加する
- `ElementTree` は scroll-view element ごとに offset を保持する（外から `element_set_scroll_offset` でセットされた値）
- `scene_build::walk()` が scroll-view を検出したとき、子の座標を `-offset` 分だけ平行移動しクリップ矩形を適用する
- `position: sticky` を持つ子要素は同じ scroll offset を使って `scene_build` 内でクランプ位置を計算する
- HTML Mode では scroll-view に対応する div の `scrollLeft` / `scrollTop` を直接設定する
- イナーシャ・スナップ等の UX 演出は Hayabusa の責務。Hayate は変更しない
