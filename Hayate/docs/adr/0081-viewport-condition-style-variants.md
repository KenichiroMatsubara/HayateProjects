# Viewport Condition: スタイルのレスポンシブ対応は Core 側でプロパティ単位の variant として評価する

**Status: accepted**

現状 Hayate CSS / Element Layer にはビューポートサイズに応じたスタイル切り替え（CSSの`@media`相当）が存在しない。これを追加するにあたり、(1) どこで評価するか、(2) どの粒度でモデル化するか、(3) 条件の表現力をどこまで持たせるかを決定する。

`effective_visual` resolver は継承（ch1+ch2）→ 自身 → pseudo (`focus<hover<active`) の順で1つの shared resolver に集約されている（spec §02 ELEM-xx）。Viewport Condition はこの resolver にもう1段の解決軸として合成する**プロパティ単位のvariant**として実装する。各 style tag は「base値 + (条件, 値) のリスト」を持ち、条件は1エントリ内で `min-width`/`max-width`/`min-height`/`max-height`（px固定、すべてAND評価）を自由に組み合わせられる。複数条件が同時にマッチする場合は宣言順で後勝ち（CSSの`@media`カスケードに準拠）。

Wire format は ADR-0052 の `apply_mutations(ops, styles, texts)` の延長として、`OP_SET_STYLE_VARIANT(op, id, kind, min_w, max_w, min_h, max_h, value...)` を新設する。条件を持たない軸は sentinel値で「unset」を表す。ops stream上の出現順序がそのままカスケード優先順位になる。

ビューポートサイズの変化検知は ADR-0080（Platform Adapter の自己配線）に基づき `hayate-adapter-web` が `ResizeObserver` 等で自動的に `on_resize` を呼び、Core側で該当プロパティを再解決する。

## Considered Options

- **Host側評価**（`resize` イベントを受けた host が条件判定し通常の `StylePatch` を送り直す）: 既存機構の変更が不要で実装コストは低いが、レスポンシブのロジックが host ごとにアドホックに分散し、Hayate CSS の一部として宣言できなくなる。却下。
- **ルールブロック単位**（要素に「条件 → 複数プロパティのスタイルセット」を複数アタッチ）: CSSの`@media`に近い書き味だが、`apply_mutations`のプロパティ単位op streamと整合せず、結局Core内部ではプロパティ単位に展開する必要がある。宣言側API（Hayabusa/Tsubameテンプレート層）の糖衣構文として将来追加する余地は残すが、Protocol Contractの正本はプロパティ単位とする。
- **Container Query（要素自身の確定サイズに基づく条件）**: layout pass の収束性（自分のサイズで自分のスタイルが変わる循環）に大きな設計変更を要するため、今回はスコープ外。Viewport（ルートサーフェス）基準のみ採用。

## Consequences

- Protocol Contract（`style_tags.json` 等）と codegen（Rust `dom_style_mapper.rs` / TS `catalog.ts`）に `OP_SET_STYLE_VARIANT` 関連の型・エンコードを追加する必要がある。
- `ElementTree` は (element_id, style_kind) ごとに base値 + variantリストを保持し、`effective_visual` resolver が現在のviewport幅/高さに対して該当する最後のvariantを採用する。
- DOM Renderer 側では `domCss` に対応する `@media` 出力へのマッピングが別途必要になる。
