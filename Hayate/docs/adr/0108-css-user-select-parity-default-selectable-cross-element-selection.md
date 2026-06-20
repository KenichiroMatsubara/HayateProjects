---
status: accepted
---

# CSS `user-select` パリティ：選択は element-kind UA 既定で「既定選択可」、要素をまたぐ document-order 選択、`contains` 境界

**Date: 2026-06-18**

> 本 ADR は ADR-0097 の**決定2（`selectable` opt-in 領域）を supersede** する。ADR-0097
> の他の決定（1: core 所有の単一 Selection モデル、3: chrome を core 描画＋テーマ切替、
> 4: 既存 intake に選択ドラッグを乗せる）は有効。決定5（DOM ネイティブ選択 + `user-select`
> マップ + 意味論のみ parity）は本 ADR で**精緻化**する（極性反転・`contains` 追加）。

## Context

ADR-0097 は選択可能性を **`selectable` boolean の opt-in 領域**（Selection Region）で
表し、「宣言した subtree 内でのみ選択でき、外には出られない」モデルにした。「全 text を
既定で選択可能」は *ボタンラベル等の UI chrome まで選択対象になる* ことを理由に却下されて
いた。

この設計には2つの問題がある:

1. **CSS `user-select` との意味論不一致 → 学習コスト。** ブラウザはテキストを既定で選択可と
   し、`user-select: none` で**除外**する（opt-out）。Hayate CSS は CSS の対応サブセットを
   謳う一方、選択だけ極性が逆（opt-in）で、開発者が既知の `user-select` を裏切る。選択可否は
   end-user 体験（コピー可否）に直結するため、語彙やカスケードのような authoring 時のみの
   意図的乖離とは質が違い、parity 側に倒す価値がある。
2. **実装が単一要素に閉じている。** `Selection::range_within(element)` は両端が同一要素の
   ときだけレンジを返す（"single-IFC tracer; cross-element is a growth point"）。ADR-0097 が
   文言で約束した「複数 block をまたぐ選択」は未達で、ハイライトもコピーも単一要素止まり。

却下理由（chrome 選択）は、実はブラウザでも「全部選択可」ではなく **UA スタイルシートが
button 等に `user-select` を効かせて選択不可にしている**ことで回避されている。つまり
element-kind ごとの UA 既定を core が供給すれば（ADR-0105 のカーソルと同型）、parity を保った
まま chrome 選択を防げ、opt-in は不要になる。

## Decision

1. **選択可能性は element-kind の UA 既定で「既定選択可」にする（opt-out）。**
   `proto/spec/element_kinds.json` に `defaultUserSelect` を追加し、単一正本として Canvas /
   DOM 双方が参照する（ADR-0105 の `defaultCursor` と同型）。既定: **text / view /
   scroll-view = `text`（選択可）**、**button / image = `none`**、**text-input = 編集選択を
   `EditState` が所有（document 選択とは別系統で常に選択可）**。

2. **`selectable` boolean を廃止し、CSS 同名の `user-select` typed property に置換する。**
   閉じた値語彙（ADR-0071）は `text | none | contains`。解決順は **明示 `user-select` →
   element-kind UA 既定 → 継承**で、`none` は subtree を選択不可にする。`all` / `auto` は
   additive な将来拡張とし当面持たない。

3. **既定では選択は要素をまたいで自由に広がる（境界なし）。`user-select: contains` を持つ
   block box だけが封じ込め境界（Selection Region）を確立する。** `contains` 内で始まった
   選択はその外に出られない。nested は最内の `contains` が有効。

4. **選択を Canonical Tree の document order（pre-order DFS）上の連続レンジとして再定義し、
   cross-element 化する。** 単一の order comparator を設け、(a) anchor/focus の正規化、
   (b) 要素ごとの塗り範囲解決（中間要素は全長・両端は部分）、(c) `contains` クランプ、
   (d) テキスト取得が**すべて同じ comparator を参照**する（重複ロジック禁止）。`range_within`
   の単一要素制約を解く。

5. **ハイライト span とテキスト取得は不可分で着地させる。** cross-element ハイライトだけ先行
   して取得を単一要素のまま残す中間状態を作らない（「またげるのにコピーできない」新規の
   非 CSS 挙動を防ぐ。現状が単一要素なので regression は無い）。テキスト取得は document order
   で範囲内 text 要素を連結し、**block box（IFC root）境界に `\n` を1つ挿入**する（ブラウザの
   コピー挙動と同型。複数改行の block 種別分けは将来）。

6. **DOM 経路は引き続きブラウザネイティブ選択 + `user-select` マップ（意味論のみ parity）。**
   `resolve_user_select` を「明示 → kind 既定」に書き換え、`fixtures/user_select_parity.json`
   の期待値を新極性に更新する（ADR-0070 の単一正本で Rust / TS 両側を pin）。`contains` 非対応
   ブラウザでは core 側の境界クランプで補完する。

## Considered Options

- **ADR-0097 の opt-in 領域を維持** — CSS と極性が逆のままで学習コストが残り、cross-element も
  未達。本 ADR の動機そのものを残すため却下。
- **極性だけ反転し境界・cross-IFC は据え置き（部分採用）** — 「またげるのにコピーは単一要素」
  という*別の*非 CSS 挙動を新規に生み、学習コストを二重化する罠。決定5で明示的に却下。
- **`user-select` ではなく独自プロパティ名で opt-out を表現** — 極性は直っても名前で CSS を
  裏切り続ける。あなたの動機（CSS 一致）に反するため却下。`all` を含む全 CSS 値の即時採用も
  語彙肥大として却下（additive 拡張に回す）。

## Consequences

- **proto/wire 契約の破壊的変更。** `element_properties.json`（`selectable`→`user-select`）・
  `element_kinds.json`（`defaultUserSelect`）・`opcodes.json`・生成コード（Rust / TS）を更新。
- `crates/core/src/element/selection.rs` に document-order comparator と cross-element レンジを
  新設し、ハイライト lowering・テキスト取得・a11y・`contains` クランプを接続。
- `adapters/web/src/user_select.rs` と `renderer-dom/src/user-select.ts` を新解決順に。
  `fixtures/user_select_parity.json` の期待値を反転（view/text 既定 `none`→`text`、button は
  `none` 維持）。
- 既存の `selectable` 利用（`examples/todo`・CSS gallery・各テスト）を `user-select` に移行。
- ADR-0097 が defer した cross-element / cross-IFC 選択の成長点を本 ADR が埋める。a11y inbound
  `SetTextSelection`（ADR-0098 defer）や `selection-change` イベントは引き続き範囲外。

## 実装ノート（2026-06-21・Canvas 経路の選択開始 / カーソル結線）

決定 1/3 を Canvas 経路で完遂した。それまで core は `user-select` typed property・`contains`
境界・`none` subtree 除外までは実装していたが、**「選択可能性 = effective `user-select`」を
カーソルと選択開始が消費していなかった**ため、明示 `selectable` / `contains` 領域を持たない
素のテキスト段落で2つの不具合が残っていた:

1. **ホバーで I-beam にならない。** `resolve_cursor`（`interaction.rs`）が「選択可能テキスト」の
   proxy に旧 `selectable` Selection Region ルートを使っていた。effective `user-select == text`
   かつ text-bearing（`is_text_like`）な要素で `text`（I-beam）を返すよう変更（ADR-0105 の
   「選択可能テキスト = text」を user-select で正しく判定）。空 `view`（kind 既定 `text` だが
   非 text-bearing）は矢印のまま、`user-select: none` テキストは I-beam を出さない。

2. **ドラッグで選択が始まらない。** 選択開始（`begin_selection_at` → `within_selectable`）が
   Selection Region ルートの存在を要求していた。決定3「既定は境界なしで自由に広がる」に従い、
   `within_selectable` を「effective `user-select` が `none` でない（= `!user_select_excludes`）」
   へ緩和。`image` / `button`（kind 既定 `none`）と `user-select: none` subtree は引き続き選択
   不可。`set_selection_range`（プログラム API）も同極性へ更新。

設計の単一アクセサ化: `el.user_select` フィールドを要素生成時に `kind.default_user_select()`
で初期化し、フィールドが effective 値そのものを保持するようにした（`drive_ime` の単一述語化
（#392）と同型）。`selection_region_of`（`contains` / legacy `selectable`）は**封じ込め境界**
専用に役割を限定し、`None` 境界は「無境界の document 領域」として扱う（`None == None` が同一
領域 → 自由に cross-element 選択）。`selectable` boolean API は移行までの境界マーカーとして存続
（決定2 の typed property 置換は別タスク）。DOM 経路はブラウザネイティブ選択 + UA `user-select`
で元から既定選択可・I-beam なので変更不要（Canvas を DOM/ブラウザ既存挙動に揃えた = parity）。

回帰固定: `crates/core/tests/plain_text_selection.rs`（新規）が plain text の I-beam / 選択 /
コピー / 非 text-bearing view の矢印 / `user-select: none` 除外を、`text_selection.rs` と
`selection_api.rs` が旧 ADR-0097 前提（領域無し=非選択）から新挙動へ更新済み。実ブラウザは
`Tsubame/examples/todo/e2e/canvas-text-cursor.spec.ts` が Canvas のカーソル結線を確認。

## 関係

- ADR-0097（統一テキスト選択）: 決定2 を supersede、決定5 を精緻化、決定1/3/4 は存続。
- ADR-0105（element-kind UA 既定カーソル）: `defaultUserSelect` は `defaultCursor` と同型の
  単一正本パターン。
- ADR-0071（closed vocabulary）: `user-select` 値はその一員。
- ADR-0070（生成マッパー / 単一正本 fixture）: parity を `user_select_parity.json` で pin。
- ADR-0104（PointerKind / modality 依存の選択ライフサイクル）: 直交、不変。
