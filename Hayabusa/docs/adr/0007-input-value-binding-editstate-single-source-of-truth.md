# `text-input` の value 束縛：編集中の単一正本は Hayate `EditState`、signal はミラー

status: accepted

Date: 2026-06-23

## Context

Todo 系デモのために `text-input` の controlled な `value` 束縛と `on:input` / `on:submit` が要る
（pending-decisions P4）。素朴な実装は React 系の「完全 controlled input」——signal を value の正本に
据え、入力イベントのたびに signal の値を要素へ書き戻す——だが、Hayate はこの前提と衝突する。

Hayate core が編集モデルの正本を**既に完全所有**している（Hayate ADR-0069 / spec TEXT-09）：

- `EditState` が `text_content` / `preedit`（IME 組成中テキスト）/ `cursor_byte_index` を保持する。
- insert / backspace / commit / paste 等の編集セマンティクスも core（`interaction.rs`）が持つ。
- IME 組成・候補窓位置（character bounds）も EditState ＋ `ImeBridge` を起点に core が握る。

ここに Hayabusa の `value` signal を「もう一つの正本」として置くと**二重正本**になり、特に IME 組成中に
signal を要素へ書き戻すと EditState の preedit / cursor を破壊する（controlled input + IME の典型破綻）。

決めるべきは「編集中、value の単一正本はどちらか」。

## Decision

**二重正本を作らない。編集中の単一正本は Hayate core の `EditState`。** Hayabusa の `value` signal は
その正本に並ぶものではなく、controlled の体験を次の非対称で実現する。

- **読み（主）**：`on:input` が commit 済みの `text_content` を Event Delivery で運び、Hayabusa が
  受け取って signal を更新する。経路は ADR-0117 の push 型 DeliverySink そのもの（App Host が drain →
  Hayabusa の `ListenerId → handler` map → handler が signal 更新 → flush）。
- **書き（従・programmatic set のみ）**：`value={signal}` は programmatic な value 設定に限る
  （例：`on:submit` 後に signal を空にしてフォームをクリアする）。Hayate がこれを EditState と
  突き合わせ、**現在値と差分があり、かつ IME 組成中でないときだけ**適用する。毎キーストロークでは
  書き戻さない。
- 結果として、IME 組成・カーソル・選択は EditState が一手に握り、signal は編集結果のミラー兼
  programmatic リセット口になる。

sink / Template IR には、この programmatic value set を表す op（差分・非組成中ガード付き）を足す。

## Considered Options

- **完全 controlled（signal が value の正本、毎入力で要素へ書き戻す）**：React 系の馴染んだモデルだが、
  Hayate が EditState で編集正本を持つため二重正本になり、IME 組成中の書き戻しが preedit / cursor を
  壊す。組成境界の特別扱いを Hayabusa 側に持ち込む複雑さに見合わない。却下。
- **完全 uncontrolled（signal は初期値のみ、以後 DOM 任せ）**：実装は最小だが、submit 後のクリアや
  プログラムからの値設定ができず Todo デモに不足。却下。
- **EditState 単一正本・signal ミラー（採用）**：編集中の正本を EditState に一本化し、`on:input`＝読み・
  `value=` programmatic set＝書き、の非対称で controlled 体験を出す。二重正本も IME 破綻も避ける。

## Consequences

- 入力束縛 API は「`on:input` で signal が更新される」「`value=` は programmatic set」という非対称を
  前提にする。「signal を変えれば即座にキャレット位置の文字が置き換わる」ことは期待できない（し、
  IME 組成中は反映されない）。これは controlled の馴染んだ直感とずれるため本 ADR に残す。
- programmatic value set は EditState との突き合わせ（差分判定・組成中ガード）を要するので、単なる
  `set_text` ではなく専用 op として sink/IR に置く。
- `on:submit` のような form レベルのイベントも DeliverySink 経路（ADR-0117）に載る。

## 実装（2026-06-23）

- **ガード（core / Hayate）**：`ElementTree::element_set_text_content_if_idle(id, text) -> bool`。
  `EditState.text_content` と差分があり、かつ `preedit.is_none()`（非組成中）のときだけ `edit.set` を
  適用し `true` を返す。毎キーストロークの echo（signal == text_content）と IME 組成中はここで no-op。
- **sink op（Hayabusa）**：`ElementSink::set_value` ＋ `Mutation::SetValue`。`HayateSink` と
  `apply_mutation` が上記 core ガードへ写す。`RecordingSink` はガード無しで記録（配線観測用）。
- **Template IR**：`TemplateNode::bind_value`（programmatic value 束縛 = 書き・従）と `on_input`
  （commit テキストを payload で受ける = 読み・主）。`on:click` と handler 列を共有。
- **配送**：`HayabusaApp` が `DocumentEventKind::TextInput` listener を登録し、`Event::TextInput` を
  `Instance::input(elid, text)` へルーティング。`value` 束縛 Effect は signal 変化で `set_value` を
  発行し、core ガードが echo を抑止する。
- **codegen**：`.hybs` の `on:input={h}` / `value={expr}` を解釈（`components/text_field.hybs`）。
- 検証：core `value_guard_tests` / `tests/input_binding.rs` / `tests/app_host.rs`（実 EditState 越しの
  読み・書き・clear）/ `tests/hybs_codegen.rs`。

## 関係

- ADR-0069（Hayate）：preedit を `EditState` に集約。本 ADR が「編集正本 = EditState」を Hayabusa 側から再確認。
- ADR-0117：push 型 DeliverySink。`on:input` / `on:submit` の配送経路はこれに乗る。
- ADR-0045：Hayabusa は hayate-core 直リンク。programmatic value set は in-process projection で発行する。
