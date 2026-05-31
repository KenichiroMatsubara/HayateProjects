# Hayabusa は Rust フレームワークとして hayate-core に直接リンクし、多言語は言語ランタイム埋め込みで実現する

status: accepted  
supersedes: ADR-0024

Hayabusa は Rust クレートとして hayate-core に直接依存する。WIT 境界を経由しない。
多言語サポート（TypeScript / Python 等）はフレームワーク内部に言語ランタイムを埋め込む形で実現し
（QuickJS または Deno Core for TypeScript、PyO3 for Python）、
Signal グラフの追跡・伝播・スケジューリングは Hayabusa Rust コアが一元的に担う。

## なぜ Rust フレームワークでなければならないか

フレームワークの言語選択が多言語対応の幅を決定する。

**Rust フレームワーク（Hayabusa）の場合：**

```
TS script  → [QuickJS 埋め込み] ─┐
Rust script → native             ─┤→ Hayabusa Rust core → hayate-core（直接 crate）
Py script  → [PyO3 埋め込み]    ─┘
                 ↑
         言語境界はここ1回
         Hayate への境界ゼロ
```

フレームワークが Rust なので hayate-core への呼び出しコストはゼロ。
言語ランタイムの埋め込みにより、スクリプト言語の種類に関わらず Signal 処理を Rust で一元管理できる。

**Rust 以外のフレームワーク（仮に Python で実装した場合）の場合：**

```
Py script → Py framework → [WIT] → Hayate
```

そのフレームワークが扱えるスクリプト言語は Python のみに限定される。
他言語スクリプトを追加しようとすると、さらに言語境界が増えバッチ処理が2回必要になる：

```
他言語 → [bridge] → Py framework → [WIT] → Hayate
          バッチ①                   バッチ②
```

Hayate への WIT 境界（②）は避けられない上に、フレームワーク内部の言語橋渡し（①）が加わる。

## Tsubame との対称性

Tsubame（ADR-0040）は Signal ランタイムを完全に放棄し、既存 JS フレームワーク（SolidJS / React / Vue）の Signal をそのまま利用する。Tsubame が持つのは Renderer Protocol と apply_mutations によるバッチのみ。

Hayabusa は Signal ランタイムを所有する。Tsubame は所有しない。この非対称は意図的である。

## Consequences

- ADR-0024 は破棄。Hayabusa Signal ランタイムの WIT 公開は行わない
- Hayabusa は Hayate WIT の外側に存在する。Hayate WIT は Hayabusa を知らず、Hayabusa が hayate-core に直接依存する一方向依存
- TypeScript スクリプトのサポートには QuickJS 等の JS エンジンを Rust プロセスに埋め込む
- Hayabusa を使わず WIT 直叩きで独自フレームワークを作る場合、そのフレームワークが扱える言語はフレームワーク自身の言語のみになる（これは Hayabusa を使わない選択であり制約として受け入れる）
