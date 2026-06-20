# 自作 fine-grained リアクティブコア：glitch-free 実行・閉じた値モデル・所有スコープ

status: accepted

Hayabusa は Signal / Computed / Effect を**自作の fine-grained リアクティブコア**として実装する
（依存の自動追跡を持つ）。外部リアクティブライブラリに委ねず、ランタイムが所有する
（ADR-0001 の責務線：reactive 機構は再利用可能でランタイム所有）。

## 実行セマンティクス

- **glitch-free**：トポロジカル順の push-mark / pull-evaluate。computed は lazy（read 時に評価）。
  菱形依存でも中間の不整合状態を描画に出さない。
- **flush 合体**：1イベント／1フレームで1回だけ flush し、ElementTree の mutation を
  まとめて1回の描画に落とす。Hayate の apply_mutations「1バッチ／frame」哲学に整合。

## 閉じた値モデル

- signal が保持できる値は**閉じた集合** number / string / bool / list / record（string キー）。
  ランタイムが所有し、DSL はこれを script コールバックなしで評価できる
  （`item.name` のようなプロパティアクセスをランタイムが理解する）。
- script の native 値は境界（ADR-0002）で marshal する。
- marshal 不可能な native 値（巨大オブジェクト・FFI ハンドル等）に限り、
  **不透明ハンドル**を escape として signal に載せる。DSL は触らず script 本体だけが扱う。
- Hayate の「閉じた typed 語彙」（renderer-independent CSS 等）と同じ思想。

## 所有スコープ（Scope・ランタイム内部の実装メカニズム）

computed / effect / signal / 子コンポーネントインスタンス / Resource の**生存と破棄**を
束ねる、ランタイム内部の**所有階層**のノードを「Scope」と呼ぶ。

- component instance 境界や `:if` / `:each` のブランチで入れ子になる
- cleanup（on_destroy・effect 破棄）、Store の参照範囲、Suspense / ErrorBoundary の集約は
  すべてこの **Scope 階層**を基準にする（ADR-0004 / ADR-0005）
- Canonical Tree（要素の親子）でも依存グラフ（DAG）でもない、独立した階層
- ランタイム本体は1つ、Scope は多数（UI 変化で動的に生成・破棄）。
  例：`:each` で N 行描けば Scope は N 個生まれ、行を消せばその Scope だけ teardown される

Scope は**ドメイン語彙ではなく実装メカニズム**であり、glossary（CONTEXT.md）には載せない。
ユーザーが触れるのは Component / lifecycle / Store / Suspense / Resource であって、
Scope はそれらをランタイムがどう実装するかである。

## Consequences

- 状態保存（hot-reload・`:each` 並べ替え）は Scope identity を基準に行える（ADR-0004 / 0006）
- 値が閉じているため DSL 評価・ABI marshal・suspense 集約が一様に機械化できる
- flush 合体により「ハンドラ内で signal を複数回触っても1描画」が既定で正しい
