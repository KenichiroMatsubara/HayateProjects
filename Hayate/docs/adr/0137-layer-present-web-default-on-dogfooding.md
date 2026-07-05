# layer-present の Web 既定を ON にする — 性能実証を待たず dogfooding を優先

**Status: accepted**

**Date: 2026-07-05**

## Context

ADR-0135 は layer-present を「製品としては有効化禁止・既定は必ずOFF」として封印し、再開条件を「(1) 実ブラウザでの描画バグ修正」と「(2) 実機/実ブラウザで計測可能な性能上の実害の実証」の**両方**とした。ADR-0136 は (1) を vello バックエンドについて満たした（premultiplied/straight alpha 取り違えの修正、box-shadow を含む回帰テスト追加）。(2) は未解消のまま残っている。

layer-present を実ブラウザで動かせる唯一の場所は、デプロイ済み Tsubame todo サンプル（Pages）に `9ba1718` で追加した本人調査用トグル（`?layerPresent=1`、既定OFF、既知バグの警告表示付き）である。issue #704（ADR-0127 の overscan サイジング配線の可否を決める議論）を進める中で、「そもそもこのサイトの利用者・検証者は本人のみであり、ADR-0135 が想定した『不特定多数が製品として触れる際の安全性』という前提がここには当てはまらないのではないか」という論点が浮上した。

## Decision

**Web（Vello バックエンド、Tsubame todo サンプルの Pages デプロイ）に限り、layer-present の既定を ON にする。native（Android/iOS）は ADR-0135 のまま既定 OFF・封印継続とし、本 ADR では変更しない。**

理由: 目視でわかる描画バグは、既定OFFの調査用トグルの奥に隠しておくより、既定として公開して自分の目に触れる機会を増やしたほうが早く見つかり早く直る（dogfooding優先）。ADR-0135 の第二再開条件（性能上の実害の実証）は Web に関しては待たない — この site の唯一の利用者・検証者は開発者本人であり、"製品として有効化する際に不特定多数を壊れた描画に晒さない" という ADR-0135 の安全側の前提がそもそも成立しないため。

具体的な変更:

- **既定値**: Web は ON。native は ADR-0135 のまま OFF（変更なし）。
- **逃げ道トグルは維持**: `Hayate/host/src/index.ts` の `layerPresent` オプション、`Tsubame/examples/todo/src/main.tsx` の `?layerPresent` クエリパラメータ読み取り、`index.html` の「最適化」行によるトグル UI は削除せず残す。デフォルト値だけを反転させ、全面raster版に戻して比較できる経路として使う。
- **警告UIは撤去**: `Tsubame/examples/todo/index.html` の「⚠️ 実験的機能: 既知の描画バグあり（ADR-0135・非推奨）」という警告色・title/aria-label の注意書きは削除する。既定経路として扱う以上、常時警告を出す必要はない（検証者は本人のみで、既知バグを理解した上での判断であるため）。

## Considered Options

- **ADR-0135 原案通り、性能実証を待ってから解禁**: 安全側だが、"性能上の実害を計測で示す" ための環境整備自体が後回しになりやすく、その間に描画バグを実地で見つける機会（dogfooding）が失われる。issue #704 のきっかけ（overscan が配線されていないことに #699 の調査でようやく気づいた）自体がこのパターンの実例。却下。
- **既存の調査用トグルのまま既定OFFを継続**: 安全側だが、本人が偶然クエリパラメータを付けない限りバグに気づく機会が少ない。却下。

## Consequences

- Web では layer-present が正式な既定経路になる。ADR-0125 の Phase 2 ロールアウト（バックエンド半分）は Web に関して事実上再開される。native（Phase 2 以降）は ADR-0135 のまま封印継続。
- 既知でない描画バグが今後も見つかる可能性があるが、それを検出する主経路がこの既定 ON 化そのものになる。
- ADR-0127 の overscan サイジング配線（issue #704 で (a) と決定、実装は別issueへ切り出し）とは独立。overscan が未配線でも既存の GPU 予算＋LRU 退避で動作するため、この既定 ON 化を妨げない。
- 警告UI撤去後、既知バグに気づかないまま使い続けるリスクはあるが、検証者が本人のみという前提を踏まえて許容する。

## 関係

- **amends** ADR-0135（layer-present 封印）— Web に関する既定OFF・第二再開条件（性能実証待ち）をこの ADR が上書きする。native 側の封印方針は変更しない。
- **references** ADR-0125（Phase 2 backend half）, ADR-0127（memory budget / scroll overscan、配線は別issueで実施予定）, ADR-0136（描画バグ根本原因・修正）。
- 動機となった議論: issue #704（`/grill-with-docs` セッション）。
