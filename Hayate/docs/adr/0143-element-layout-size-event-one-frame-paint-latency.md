# per-element layout size イベントを新設し、wire 経路の painter は 1 フレーム遅延を受容する

**Status: accepted**

**Date: 2026-07-06**

## Context

draw（ADR-0141）の painter は `paint(canvas, size)` でレイアウト確定サイズを受け取る（flex で決まる box に絵が追従するため。Flutter `CustomPainter` と同型）。しかし wire 経路ではレイアウトは WASM 側 `commit_frame` の中で確定し、JS は要素の確定サイズを知る口を持たない——既存の `resize` イベントは viewport 全体の host-echo のみで、per-element の layout 通知は契約に存在しない。Flutter で `paint(canvas, size)` が成立するのはレイアウトとペイントが同一プロセス・同一フレームで直列だからである。

## Decision

**要素のボーダーボックスサイズが確定/変化したときに Event Delivery で届く per-element layout size イベントを event 語彙に追加する**（ブラウザ ResizeObserver 相当。draw 専用ではなく汎用イベント。リスナ登録要素のみ発火・論理 px）。

Tsubame はこのイベント受信時（初回確定・サイズ変化）に painter を実サイズで呼び、記録した display list を**次フレーム**の mutation で送る。結果として**初回マウント・リサイズ時に描画が 1 フレーム遅れる**ことを仕様として受容する。これはブラウザの ResizeObserver + canvas 再描画とまったく同じ意味論であり、web では標準的な挙動。

将来の Hayabusa in-process 経路では painter をレイアウト直後に同一フレーム内で呼べる。契約モデルは同一のままタイミングだけ良くなる非対称は許容する。

## Considered Options

- **size 非依存の記録（正規化座標や宣言サイズ基準で記録し、実サイズへは Hayate 側で伸縮）**: 遅延は消えるが、painter が size で分岐できず（Flutter 同等の表現力を最初から損なう）、伸縮で stroke 幅が歪む。却下。
- **同期レイアウト問い合わせ（JS から要素サイズを同期 query）**: フレーム内で mutation → layout → query → mutation の往復を要求し、`tick` の単一 flush 点（ADR-0117）と HayateRenderer 所有 semantic queue のフレーム単位バッチ契約を壊す。却下。

## Consequences

- per-element resize 観測は draw 以外（可視化・仮想化等）にも使える汎用の契約資産になる。
- イベントの正確な命名・`wireRole` 分類は既存 spec 慣行に従い実装時に確定する。
