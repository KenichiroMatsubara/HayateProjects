# `platform/` — Capability Grouping Doctrine（ADR-0117）

このディレクトリは ADR-0117 の三層モデル（Core / Family Adapter / leaf）における **adapter 層の
実体置き場**である。本 README は capability を三段階へ振り分ける **grouping doctrine（振り分け
規律）の正本**。語彙の定義は [`../CONTEXT.md`](../CONTEXT.md) の Platform Adapter / Family Adapter
/ Capability を参照。

> **Capability とは:** 各 OS のネイティブ API 呼び出しが**必須**な機能（audio / clipboard /
> notification / haptics / file picker 等）。platform-free な共通ロジック（touch gesture / surface
> 状態機械 / IME 増分 = **Core 所有**）とは別物。混同しない。

## ディレクトリ構成

```
platform/
├── common/              # 全 platform 共通 capability 実装の置き場（枠・現状 leaf 実装なし）
├── mobile/              # mobile Family Adapter（android + ios の cfg facade）
│   ├── android/         #   leaf（Android）
│   └── ios/             #   leaf（iOS）
├── desktop/             # desktop Family Adapter の枠（leaf 0・capability trait 未作成）
└── web/                 # leaf（Web・family of 1 なので Family Adapter を持たない）
```

`web` は単一 platform（family of 1）なので Family Adapter を持たず leaf を直接置く。`desktop` は
leaf が 0 のため**枠（ディレクトリ + 本 doctrine）だけ**を前払いで用意する（[`desktop/README.md`](desktop/README.md)）。

## 三段階の振り分け規則

capability は**共通度**で次の三段階に振り分ける。判定は「どこまでの platform で**同一契約**を
満たせるか」で行う。

| 段 | 置き場 | 意味 | 例 |
| --- | --- | --- | --- |
| **全 platform 共通** | `platform/common/` | web / mobile / desktop すべてで同型に供給できる | （現状なし — 昇格は 2 実装が揃ってから） |
| **family 共通** | `platform/mobile/` ・ `platform/desktop/` | family（mobile = android+ios / desktop = macos+windows+linux）内で統一でき、family を跨ぐと割れる | audio（mobile: AudioTrack / AVAudioEngine） |
| **leaf 固有** | `platform/web` ・ `platform/mobile/<os>` ・（将来）`platform/desktop/<os>` | その OS でしか意味を持たない、または OS ごとに振る舞いが大きく割れる | web clipboard（DOM `navigator.clipboard`） |

判定の方向は **下から上へ昇格**：まず leaf 固有として実装し、独立した 2 実装で variation が
確認できてから family / common へ持ち上げる。最初から「共通」と決め打って上段へ置かない。

## Flutter / RN taxonomy からの分類例

借りるのは **taxonomy（カタログ）だけ**。Flutter plugin / RN（React Native）TurboModule が
ネイティブ機能をどう分類しているかという**prior art のカタログ**を写し、各機能がどの段に属する
かの初期見積りに使う。**機構（channel / bridge のランタイム dispatch）は借りない**。

| capability | 妥当な段（初期見積り） | 補足 |
| --- | --- | --- |
| **audio**（再生・出力） | family 共通 | mobile は AudioTrack / AVAudioEngine で統一可。desktop は別 family。すでに `platform/mobile` の facade に載る唯一の確定 capability。 |
| **clipboard** | （capability 化しない） | ADR-0014 の Platform Adapter 責務として ADR-0097 が編集境界 `element::clipboard::Clipboard`（選択テキストのコピペ）で所有済み。同一 OS API への 2 重抽象を避けるため capability scaffold には含めない（ADR-0119）。 |
| **notification** | family 共通 | 権限モデル・表示機構が family 内で近く、family を跨ぐと割れる。 |
| **haptics**（触覚） | leaf 固有 → family 共通 | desktop には概念が薄い（leaf 固有寄り）。mobile では family 統一余地。 |
| **file picker** | family 共通 | OS のシステム picker を呼ぶ。UI 機構は family ごと、契約（選ばれた path/stream）は共通化しやすい。 |

> これらは**初期見積り**であり、契約の正本ではない。実装で variation が判明したら段は調整する
> （ADR-0117 が受容するリスク）。

## 三つの規律（doctrine の芯）

1. **契約の正本は常に Core。** capability の契約（trait）は `ImeBridge` / `Surface` / `FontFetcher`
   と同型に **Core が所有**する（ADR-0068/0069）。`platform/{common,mobile,desktop}/` は capability
   の**実装と family facade の置き場**であって、契約の正本ではない。trait を adapter 側に切らない。
2. **共通 API への昇格は原則 2 実装が揃ってから。** 1 実装だけの seam は仮説にすぎない（ADR-0068
   の投機 seam 戒め）。独立した 2 実装で variation を確認してから上段（family / common）へ昇格する。
   例外は ADR-0068 の前払い条件を満たす場合のみ（desktop の枠 = variation 確定済み + 確定ターゲット）。
3. **借りるのは taxonomy のみ、機構は借りない。** Flutter channel / RN bridge の**機構（ランタイム
   dispatch）は借りない**。Family Adapter はビルド時 `cfg(target_os)` で片方の leaf をリンクする
   facade である。借りるのは「どの機能がどの段に属するか」という taxonomy（カタログ）だけ。

## trait を先置きしない

枠（`common` / `desktop`）には capability trait を**先置きしない**。trait は、その capability を
実装する具体 leaf に着手したとき初めて Core へ足す（空 trait / 空 facade を置かない）。現在 trait
が存在する capability は mobile audio（android + ios の 2 実装が確定）のみ（[`mobile/`](mobile/)）。
