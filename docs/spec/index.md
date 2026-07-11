# HayateProjects 設計書（正本）

この `docs/spec/` がシステムの**規範の単一正本**である。「何であるべきか」はここに書く。「なぜそう決めたか」は各項目が参照する ADR（`*/docs/adr/`）に残る。語彙は [`CONTEXT.md`](../../CONTEXT.md)。

> **状態:** 全13パート構築済み（ROOT/Hayate/Tsubame/Hayabusa の ADR 群 ≈ 150 本（Hayate は ADR-0119 まで）から 109 要件を抽出・実装ステータス検証済み）。直近の整合更新: ADR-0004（host bootstrap 退去）・ADR-0117（三層アダプター + App Host seam）・ADR-0118（desktop winit）・ADR-0119（mobile capability scaffold）・Tsubame ADR-0010（tsubame-react）を反映。

## 読み方

各パートは規範文（要件）単位の項目で構成される。1項目のスキーマ:

| フィールド | 意味 |
|---|---|
| **ID** | パート接頭辞 + 連番（例 `PROTO-04`）。安定識別子。 |
| **規範文** | 現在形・テスト可能な契約一文。 |
| **出典** | 根拠 ADR（n対多）。supersededなADRは項目化せず履歴として備考に残す。 |
| **状況** | ✅実装済み / 🟡部分 / ⬜未実装。✅🟡は file:line or テスト参照を持つ。 |
| **備考** | 矛盾・supersede履歴・Open論点。`★` は要判断項目。 |

**状況の定義** — ✅実装済み: 規範文の全要素がコードに存在しテスト or 実体で担保。🟡部分: 中核は在るが規範文の一部（検証層・方向・モード）が欠落。⬜未実装: コードに痕跡なし、または設計のみ確定で未着手。

## パート一覧

| § | パート | ✅ | 🟡 | ⬜ |
|---|---|---|---|---|
| §0 | [システム & ドキュメント運用](./00-system.md) | 3 | – | – |
| §1 | [Hayate Core 原則](./01-core.md) | 5 | – | – |
| §2 | [Element Layer](./02-element-layer.md) | 6 | – | – |
| §3 | [Layout](./03-layout.md) | 7 | – | – |
| §4 | [Raw Layer / Scene Graph / Rendering](./04-rendering.md) | 12 | 1 | 2 |
| §5 | [Text / Font / IME](./05-text-font-ime.md) | 10 | – | – |
| §6 | [Event Model](./06-event-model.md) | 9 | – | – |
| §7 | [Scroll](./07-scroll.md) | 3 | 1 | – |
| §8 | [Web Adapter & Modes](./08-web-adapter-modes.md) | 6 | 1 | 1 |
| §9 | [Platform Adapter & Accessibility](./09-platform-accessibility.md) | 5 | 4 | 2 |
| §10 | [Protocol & Wire Contract](./10-protocol-wire-contract.md) | 19 | 1 | – |
| §11 | [Tsubame](./11-tsubame.md) | 5 | 3 | – |
| §12 | [Hayabusa【凍結】](./12-hayabusa.md) | – | – | 5 |
| §13 | [Mobile Capabilities](./13-mobile-capabilities.md) | – | – | 2 |
| | **合計** | **90** | **11** | **12** |

全 **113 要件**。⬜ 12 件のうち 6 件は歴史（WEBA-08）または凍結（HAYA-01〜05）で徹底実装フェーズの対象外、2 件（PLAT-07 AccessKit inbound / PLAT-10 desktop winit leaf）は設計確定・未着手の作業対象、2 件（MOBL-01 wave-2 ストリーム capability 契約 / MOBL-02 capability 公開境界）は ADR-0120 で設計確定・後続 scaffold 待ち、2 件（REND-14 skia-safe Scene Renderer / REND-15 ネイティブ selection policy）は ADR-0146 で設計確定・実装スライスは PRD #798 の子 issue が担う。

## 実装ステータス・ダッシュボード（未完了の要件 = 徹底実装フェーズの作業対象）

### 🟡 部分実装（11件）
| ID | 規範文要約 | 欠落 |
|---|---|---|
| REND-08 | Render Host 芯の共有層 hoist | `adapter-web` 内残留（ADR-0068） |
| SCR-04 | Scrollbar Chrome（core overlay・modality 分岐） | 描画＋Mouse/Pen 操作＋Touch indicator ✅、DOM overlay 固定のみ open（ADR-0110） |
| PLAT-04 ★ | AccessKit 展開順序 | Core TreeUpdate✅、ネイティブ/Web AT 報告未着手 |
| PLAT-06 | Android ネイティブ Platform Adapter | (A)描画/(B)タッチ着手、(C)フルパリティ（IME/AccessKit/clipboard）未着手 |
| PLAT-08 | iOS ネイティブ Platform Adapter | 純粋シーム＋契約テスト✅、Metal/FFI グルー・Swift ホスト・実機 IME は Mac 検証残（ADR-0114〜0116） |
| PLAT-11 | mobile capability scaffold | wave-1 の 9 capability を型＋`Err(Unimplemented)` で scaffold✅、実機実装未着手（ADR-0119） |
| WEBA-01 | モード自動判定 | backend probe は `@torimi/hayate-host` へ移管✅、Canvas↔HTML 自動判定（EditContext probe→HTML）未配線 |
| PROTO-09 | wire codec 単一正本 | 手書き `hayate.ts` の `parseColor`/`parseDimension` 残存（dead code 削除待ち） |
| TSUB-02 | property 閉じた語彙 | `value`/`placeholder`/`disabled`/`src` ✅。`aria-label`/`role` first-class 未接続 |
| TSUB-05 | adapter は既存ランタイム持込 | solid✅・react✅（ADR-0010）、vue未実装 |
| TSUB-08 | viewport/resize は host adapter 責務 | Tsubame 退去✅、Hayate web adapter の resize 抽象化は follow-up（ADR-0004） |

### ⬜ 未実装（12件・うち2件が作業対象）
| ID | 規範文要約 | 種別 |
|---|---|---|
| REND-14 | skia-safe Scene Renderer（ネイティブ専用・surface 非依存 painter・SkTextBlob） | 設計確定（ADR-0146）・実装スライスは PRD #798 の子 issue |
| REND-15 | ネイティブ Renderer Selection Policy（vello → skia fallback・ランタイム切替） | 設計確定（ADR-0146）・前提工事は Render Host 芯 hoist（REND-08） |
| PLAT-07 | AccessKit inbound action の Core 写像 | 設計確定（ADR-0098）・ネイティブ adapter 前提で未着手（作業対象） |
| PLAT-10 | Desktop winit 単一 crate leaf | 設計確定（ADR-0118）・`hayate-platform-desktop` crate 未実装（作業対象） |
| WEBA-08 | ADR-0010/0011 は歴史的 | 歴史（実装不要） |
| HAYA-01〜05 | Hayabusa 全般 | 設計確定・将来（凍結） |
| MOBL-01 | wave-2 ストリーム capability 契約（専用契約・query/subscribe・poll-drain・RAII subscription） | 設計確定（ADR-0120）・後続 wave-2 scaffold 待ち |
| MOBL-02 | capability 公開境界（in-process DI・wire/JS 公開は延期＝blocked issue 追跡） | 設計確定（ADR-0120）・wire 公開は blocked |

## 矛盾マップ

種別: **[履歴]** = 機械的supersession/amend、ADR本文が勝者を宣言済み → 自動解決。**[衝突]** = 番号衝突等の表記問題。**[要判断]** = 真の未解決矛盾・実コードとの食い違い → grill でエスカレーション。**[解決]** = ユーザー指示等で確定。

### ★ 要判断（grill エスカレーション対象）
**現在 0 件** — 4 件の要判断（C-2.1 / C-4.1 / C-10.5 / C-10.6）はすべて grill で決着（下記 [解決]）。残る 🟡/⬜ は設計確定済みの実装作業のみ。

### [履歴]（自動解決済み）
| ID | supersede/amend | 解決先 |
|---|---|---|
| C-8.1 | ADR 番号衝突: `0028`×2 / `0029`×2 → `0073`（canvas fonts）/ `0074`（html z-index 歴史）に改番 | §5 TEXT-02 / §8 WEBA-04〜06 |
| C-0.1 | Tsubame 0001 独立リポ → ROOT 0001 モノレポ（アーキ分離は維持） | SYS-01 |
| C-1.1 | 「wgpu唯一」の文言 vs tiny-skia 併存（GPU層の規範であり CPU fallback と非矛盾） | CORE-02 / REND-11 |
| C-7.1 | 0022 上位層所有 → 0046/0053 core集約（CONTEXT.md「Scroll Offset」の 0022 参照 drift は 2026-06-09 修正済み） | SCR-01 |
| C-7.2 | ADR-0046「スクロール物理は Platform Adapter 所有・未実装」 → ADR-0113 が Core 所有へ supersede（Scroll Gesture ＋ Physics Profile `auto`/`ios`/`android`、Adapter はフレーム駆動）。spec 旧記述（物理＝Adapter・open）を実態（`scroll.rs` 実装済み）へ訂正 | SCR-01 |
| C-10.1 | WIT契約正本（0013/0015/0033/0039）→ 0049 JSON spec | PROTO-01/02 |
| C-10.2 | protocol.yaml（0049原案）→ 0053 proto/spec/*.json | PROTO-01 |
| C-10.3 | element_create batch外（0039）→ 0005 OP_CREATE=9 batch内 | PROTO-06 |
| C-10.4 | 文字列op apply_mutations外（0039）→ 0052 string table | PROTO-07 |
| C-11.2 | 0038 Tsubame=signal runtime → 0040 renderer target | TSUB-01/05 |
| C-11.3 | ElementId WASM返却 → 0005 JS採番 | TSUB-07 / PROTO-06 |
| C-12.1 | 0024 signal runtime WIT公開 → 0045 多言語埋め込み | HAYA-02 |
| C-12.2 | 0025 Hayabusa独立リポ vs ROOT 0001 モノレポ（物理併合・アーキ分離維持） | HAYA-03 |
| — | ADR-0030 両モード遅延 → 0037 Canvas撤去（HTML限定に縮小） | ELEM-04 / WEBA-02/03 |
| — | ADR-0010 → 0011（historical） | WEBA-08 |

### [解決]（ユーザー指示）
| ID | 内容 | 対応 |
|---|---|---|
| C-2.1 | text-as-element が Canvas/DOM で非対称（と旧評価） | grill で誤読と判明（両側 text element が文字列をプロパティで持つ同型）。`element_set_text` を Text/TextInput に構造化。ELEM-05 ✅ |
| C-4.1 | Z-Order の順序3分散（paint昇順 / hit-test降順 / resolved無ソート） | grill で `ordered_children` 単一 seam に集約決定。ADR-0060 / REND-03 ✅ |
| C-10.5 | ADR-0055 の検証層が apply_mutations 方向のみ（delivery 方向の共有 fixture 欠落） | grill で検証トポロジ確定。ADR-0055 amend（C5 層）/ PROTO-17 ✅ |
| C-10.6 | app font ID と font_family の接続（旧「100+ 予約帯」案） | grill で前提（数値enum）が obsolete と判明。文字列接続を確定、web fonts.json→codegen。ADR-0061 / PROTO-19 ✅ |
| C-11.1 | `tsubame-spec.md`「Tsubame は archive 化せず維持」 vs 単一正本への統合 | ユーザー指示で archive 送り。「維持」記述は破棄 |
| C-4.2 | ADR-0054「公開 API は `render_scene` のみ・walk/Painter は crate 内部」 vs 実装が `VelloPainter`/`TinySkiaPainter` を `pub use` 公開（外部利用なし） | grill（ADR↔実装乖離監査 2026-06-09）。Painter を非公開化、`straight_to_premultiplied` も非公開。ADR-0054 を amend し「公開＝`render_scene` + surface 補助」を明文化。REND-10 ✅ |
| C-8.2 | root `CONTEXT.md`「WIT＝現行の公開 API 単一ソース・Raw Layer 公開」 vs ADR-0049（WIT 廃止→JSON proto）/ ADR-0072（Raw Layer 公開棄却） | ADR↔実装乖離監査 2026-06-09。WIT エントリを【歴史】化（Hayate/CONTEXT.md と整合）。Tsubame「別リポジトリ」記述も ADR-0001 モノレポへ更新 |
| C-8.3 | §8 WEBA-01「probeWebGPU で Canvas/HTML モード自動判定」 vs 実装は GPU/CPU バックエンド選択のみ（HTML renderer は Tsubame init から到達不能） | ADR↔実装乖離監査 2026-06-09。WEBA-01 状況を実態（モード自動判定は未配線、HTML renderer は dead path）に訂正。★ Tsubame init への EditContext probe + HTML 経路追加は実装ギャップとして残置 |
| C-idx | index.md の集計・ダッシュボードが part files（§4/§7/§9/§11）と乖離（§9 は PLAT-06〜08 追加済みなのに index 行は 4/2/0 のまま 等） | Spec↔part 同期監査 2026-06-25。matrix 行・合計・🟡/⬜ ダッシュボードを part files に再同期（90✅ / 11🟡 / 8⬜・全 109 要件） |
| C-11.4 | §11 TSUB-08「ビューポートのサイズ追従は CanvasRenderer の責務（`ResizeObserver`＋DPR）」＋ PROTO-16/18「resize＝Renderer Protocol surface・`IRenderer.resize` を正」 vs ADR-0004（host bootstrap 退去・resize は host adapter 所有・CanvasRenderer は `{raw,clock}` のみの host 盲目コア） | Spec↔ADR/実装乖離監査 2026-06-25。実コード（`canvas-renderer.ts` から canvas/resize/ResizeObserver/DPR/RAF 撤去済み）を確認。TSUB-08 を host adapter 責務へ書き換え（ADR-0007 を supersede）、TSUB-02/04 を host-blind clock 注入へ、PROTO-16/18 の resize 記述を訂正、`init.ts` 退去を WEBA-01 に反映（`@torimi/hayate-host`） |
| C-11.5 | §11 TSUB-05 / §6 EVT-06「tsubame-react は ⬜未実装」 vs 実装（`@torimi/tsubame-react` の react-reconciler HostConfig 実装済み、Tsubame ADR-0010） | 監査 2026-06-25。TSUB-05/EVT-06/TSUB-01 を react 実装済みへ更新。イベント語彙拒否は renderer-protocol 共有へ移管済みも反映 |
| C-9.1 | §9 が Android（PLAT-06）/iOS（PLAT-08）のみで、desktop・三層アダプターモデル・mobile capability・App Host seam を欠く vs ADR-0117（三層 + app-host）/ADR-0118（desktop winit）/ADR-0119（mobile capability scaffold） | 監査 2026-06-25。PLAT-09（三層モデル）/PLAT-10（desktop winit leaf）/PLAT-11（mobile capability scaffold）/REND-13（App Host boot seam）を新設。「winit 不採用」を mobile native leaf 限定と明記し desktop family の winit 採用と非矛盾化（ADR-0118） |

## 運用（governance）

- この設計書が規範の正本。徹底実装フェーズは上の 🟡/⬜ 項目を作業対象とする。
- 既存 ADR は削除せず「なぜ」の記録として残置。各項目が出典 ADR へリンクする。
- 新決定: まず設計書を更新。grill 基準（不可逆 × 意外 × 真のトレードオフ）を満たす重大決定のみ ADR を追記。
- 旧 `Hayate/docs/spec.md` / `Tsubame/docs/tsubame-spec.md` / `decisions-pending.md` / `TODO.md` は archive 送り（内容は本設計書に吸収済み）。
- `CONTEXT.md` は語彙のみ。実装詳細・決定は書かない。
