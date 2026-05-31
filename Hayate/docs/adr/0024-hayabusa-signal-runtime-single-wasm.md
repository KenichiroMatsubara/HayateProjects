# Hayabusa の Signal ランタイムは単一 WASM バイナリとして実装する

status: superseded by ADR-0045

Signal グラフの追跡・伝播・スケジューリングを担うランタイムは
Hayabusa の単一 WASM バイナリとして提供する。
TypeScript / Rust / Python 等の全 Script Adapter はこのランタイムを
WIT 経由で呼び出す。言語ごとのイディオム的ラッパー
（TypeScript: `.value` アクセサ / Rust: `.get()` / Python: `.value` プロパティ等）は
各 Script Adapter が薄いラッパーとして提供するが、
グラフの実体は単一ランタイムが保持する。

## Considered Options

- **単一 WASM ランタイム（採用）**: Signal の挙動・バグ・スケジューリングポリシーの収束先が一か所。全言語で完全に同一のリアクティブ意味論が保証される。TypeScript から WASM を呼ぶオーバーヘッドが生じるが、Signal 操作の頻度はフレームあたり限られるため許容範囲と判断。
- **言語別独立実装**: 各言語で最適なパフォーマンスが得られる一方、バグの収束先が言語ごとに分散し、意味論の一貫性を仕様テストで担保し続けるコストが高い。
- **Rust WASM をリファレンスとし他言語がラップ**: 採用案と実質同一だが、「リファレンス」という表現は他言語実装の存在を示唆するため採用案の表現を選ぶ。

## Consequences

- Hayabusa ランタイム WASM は Hayate WIT とは別の WIT インターフェースを公開する
- Script Adapter は Hayate WIT（Element Layer）と Hayabusa ランタイム WIT の両方をインポートする
- TypeScript 向け Script Adapter は JS → WASM 呼び出しを含む薄いラッパーになる
- Signal ランタイムのバグ修正は全言語 Adapter に一度に反映される
