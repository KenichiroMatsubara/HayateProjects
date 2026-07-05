# tiny-skia/vello_cpu の per-layer 経路: 既定ONを維持し、比較用のランタイムトグルのみ追加する

**Status: accepted**

**Date: 2026-07-05**

## Context

issue #705: ADR-0135 の `layer-present` 封印は `platform/web/src/backend/vello.rs` の
`#[cfg(feature = "layer-present")]`（cargo feature、既定 OFF）だけをゲートしており、
`tiny_skia_backend.rs`・`vello_cpu_backend.rs` の `supports_layer_present()` はどちらも
無条件 `true` — feature flag の外にあり、封印が及んでいない。ADR-0136 の調査でこの2
バックエンドには premultiplied/straight alpha の取り違えバグ自体は無いことを確認済み
（tiny-skia は `tiny_skia::Pixmap` の premultiplied 不変条件により原理的に発生しない、
vello_cpu は最大4/255の既知の AA 丸め差のみ）。

tiny-skia は `Cargo.toml` の `default = ["backend-vello", "backend-tiny-skia"]` により
Pages にデプロイされる既定ビルドに最初から含まれ、`renderer_selection.rs` の
`PRODUCTION_RENDERERS`／選好順 `[Vello, TinySkia]` により WebGPU 非対応ブラウザの
実際のフォールバック先になる。vello_cpu は既定 features には無いが、
`Tsubame/examples/todo/index.html` の renderer 切替 UI（`auto`/`vello`/`tiny-skia`/
`vello-cpu`/`dom` ボタン）から警告表示なしに誰でも一クリックで選べる、デプロイ済みの
本番導線に既に乗っている。かつレイヤー昇格（`hayate-core::element::tree` の
`capture_frame_layers`、ADR-0125）は feature flag と無関係に常時動作するため、
transition・scroll のたびに実際に per-layer 合成コードパスを踏む。

一方で、ADR-0137（issue #704）が既に確立した前提——「このデプロイ済みサイトの
利用者・検証者は開発者本人のみであり、ADR-0135 が想定した『不特定多数が製品として
触れる際の安全性』という前提はそもそも成立しない」——は、vello の layer-present だけ
でなく tiny-skia・vello_cpu にも等しく当てはまる。ADR-0137 は同じ理由で Web の vello
layer-present の既定を ON に反転し、警告 UI を撤去した。

## Decision

**tiny-skia・vello_cpu の per-layer 経路は、既定 ON（現状維持）のまま封印しない。**
ADR-0135 が定める第一再開条件（実ブラウザでの描画バグ修正）は ADR-0136 の構造的確認
（回帰テスト・ライブラリ不変条件による論証）で足りるとみなし、第二再開条件（性能上の
実害の実証）は ADR-0137 と同じ理由で待たない——唯一の検証者が開発者本人である間は、
"製品として不特定多数を壊れた描画に晒さない" という安全側の前提が成立しないため。

vello と同じ意味での cargo feature・別 wasm-pkg は新設しない。理由は、vello の
`layer-present` feature は「perf 検証中は全面描画経路を無傷に保つ」という当時の安全策
（#690, commit `40e1ae3`）が発端であり、実ブラウザ投入時（`9ba1718`）に2つの wasm-pkg
を出し分けるしかなくなったのはその副作用に過ぎない。tiny-skia/vello_cpu には対応する
安全策上の理由が無く、既知のバグも無いので、同じ構造を複製する必然性が無い。

代わりに、renderer 切替 UI に **tiny-skia・vello_cpu 用の比較トグル**（per-layer ON/
OFF を切り替え、OFF 時は全面 `render_scene` にフォールバック）を追加する。実装は
バックエンド構造体内の**ランタイムフラグ**（`supports_layer_present()` が読む bool
フィールド）とし、新規 cargo feature・新規 wasm-pkg・#700 のビルドマニフェストへの
エントリ追加は行わない。既知のバグが無いため、vello の「最適化⚠️」トグルのような警告
UI は付けない——単に「per-layer: ON/OFF」の比較用ラベルとする。

## Considered Options

- **vello と同じ cargo feature + 別 wasm-pkg で封印**: ADR-0135 と対称的で分かりやすいが、
  既知のバグが無い状態にわざわざビルド成果物を増やす必要性が無く、ビルドマニフェスト
  （#700）へのエントリ追加という余計な作業が発生する。却下。
- **何もせず現状（gate 無し・トグル無し）を維持**: (b) 単独の選択。安全側だが、本人が
  デバッグ時に ON/OFF を見比べる手段が無くなる。開発者自身が「押せば ON/OFF が分かる
  ようにしたい」と明言しているため不採用。
- **#697 相当の専用 Playwright 実ブラウザ検証を正式タスクとして今すぐ実施 (c)**: ADR-0136
  の構造的確認より確度は上がるが、唯一の検証者が本人であり実害の実例も無い現段階では
  優先度が低い。日常使用の中での気づき（dogfooding）に委ねる。却下（今回は見送り、
  下記の再検討トリガー参照）。

## Consequences

- tiny-skia・vello_cpu は per-layer 経路が正式な既定経路であり続ける。ADR-0125 の
  Phase 2 ロールアウトは実質的にこの2バックエンドについても既に進行中だったことになる。
- renderer 切替 UI に per-layer 比較トグルが増える（vello の「最適化⚠️」列とは別に、
  tiny-skia/vello-cpu 選択時のみ有効な警告色無しの列）。
- 既知でない描画バグが今後見つかる可能性は残るが、検出手段は本人の日常使用に委ねる。
- **再検討トリガー**: 本人以外の実利用者・実検証者が想定される段階になったら、本 ADR・
  ADR-0137 の両方の「既定 ON」判断を再検討する。
- vello の `layer-present` cargo feature をランタイムフラグ化し `pkg-layer-present` を
  廃止できないか、という論点は本 ADR のスコープ外——別 issue で追う。
- tiny-skia/vello_cpu 比較トグルの実装（UI・ランタイムフラグ配線）は本 ADR のスコープ外
  ——別 issue で追う。

## 関係

- **amends** ADR-0135（layer-present 封印。tiny-skia/vello_cpu への非適用と既定 ON 継続を
  明記する）。
- **references** ADR-0136（描画バグ根本原因調査。tiny-skia/vello_cpu にバグが無いことの
  確認元）、ADR-0137（唯一の検証者は本人、という前提と既定 ON 反転の先例）、ADR-0125
  （compositing layer incremental rendering）。
- 動機となった議論: issue #705（`/grill-with-docs` セッション）。
