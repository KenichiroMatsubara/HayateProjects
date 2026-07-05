# レイヤキャッシュのメモリ予算と scroll レイヤの overscan サイジング（GPU 予算＋LRU 退避）

**Status: proposed (draft) — native は ADR-0135 により封印中（layer-present feature 有効化禁止）。Web は ADR-0137 により既定 ON へ改定済み。本 ADR の scroll overscan サイジングは設計・単体テスト済みだが実際の `present_layers`/`VelloLayerRasterizer` へは未配線（issue #704 で (a) 配線する方針を決定、実装は別issueへ切り出し）**

**Date: 2026-06-30**

## Context

ADR-0125 はレイヤを **whole-layer texture** でキャッシュすると決めた（Flutter 流）。本命ターゲットは native モバイル（Android/iOS）で GPU メモリが厳しい。問題は **スクロール内容レイヤ**：長いリスト等はビューポート高を遥かに超え、全高を 1 texture にすると（DPR² 込みで）VRAM を圧迫する。Blink がスクロールレイヤをタイル化する理由そのものである。ADR-0125 は全面タイル化を初手で行わないと決めたため、メモリの抑え方を別途定義する必要がある。

## Decision

**scroll レイヤは「可視域＋オーバースキャン余白」だけキャッシュし、全レイヤキャッシュに GPU バイト予算＋LRU 退避を課す。予算はプラットフォームが注入する。**

- **scroll レイヤサイジング**: スクロール内容レイヤの texture は全高ではなく **可視域（ビューポート）＋上下オーバースキャンマージン**だけを raster する。スクロールがマージンを超えたら、新規に現れた帯を差分 raster してキャッシュを更新する（後で入れる本格タイル化の自然な縮退版。設計を二度書きにしない）。
- **GPU 予算**: 全レイヤ texture の合計バイトに上限を設ける。単位は「ビューポート N 枚分」で表現し、**モバイル既定は小さめ（例 3–4×ビューポート）**、デスクトップ/native ハイエンドは大きめに可変。
- **退避**: 予算超過時は **最も長く composite に使われていないレイヤ texture から LRU 退避**する。退避されたレイヤは次に必要になった時に再 raster される。
- **予算の所在**: 予算値は **Platform Front / バックエンドが持つ注入パラメータ**。コア（ADR-0125）のレイヤ判定・`layer_dirty` は予算を知らず不変に保つ（policy は core、budget は platform）。

## Considered Options

- **予算なし・全レイヤ全サイズ保持**: 実装は単純だが、モバイルで退避なしに VRAM 破綻。却下。
- **初手から Blink 流の全面タイル化**: スクロール新規領域も部分更新も最小コストだが、タイル管理・per-tile dispatch・eviction の複雑性が増し、Vello では小 dispatch 多発が逆効果になり得る。計測でボトルネックが出てから（ADR-0125 の方針）。
- **予算をコアに持たせる**: プラットフォーム差（VRAM 容量）をコアに漏らし、レイヤ判定の純粋性を損なう。却下。

## Consequences

- whole-layer 判断（ADR-0125）がモバイルで生き残る前提条件が満たされる。メモリ予算/退避は v1 必須要件であり後付けにしない。
- scroll の overscan サイジングは将来のタイル化への縮退版で、移行時に設計を捨てない。
- 高速スクロールで overscan を超え続けると差分 raster が頻発し得る。マージン量はプラットフォーム注入パラメータで調整可能とする。

## 関係

- **extends** ADR-0125（compositing layer incremental rendering）。
- ADR-0113（scroll physics は core の profile）／ADR-0022（scroll offset は upper layer 所有）と整合（offset は既存所有者、キャッシュサイズだけ本 ADR）。
- 将来のタイル化 ADR がスクロールレイヤをタイル正本へ昇格する余地を残す。
