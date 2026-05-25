# WIT を単一ソースとし、Web は Wasm、ネイティブはネイティブライブラリにコンパイルする

Hayate の公開インターフェース（WIT）は一つのソースとして管理し、ターゲットごとに異なる成果物にコンパイルする。

- **Web 向け**: Wasm コンポーネントとしてコンパイル。ブラウザが提供する Wasm ランタイム上で動作する
- **ネイティブ向け**: wit-bindgen でネイティブライブラリとしてコンパイル。Wasm ランタイム（wasmtime 等）をホストプロセスに埋め込む必要がない

WIT は「インターフェースの定義書」であり、「必ず Wasm 経由で呼ぶ」という意味ではない。

## Considered Options

- **ネイティブでも Wasm ランタイム経由**: WIT という単一の呼び出し境界を保てるが、ネイティブビルドに wasmtime が入り、バイナリサイズと起動コストが増える。
- **ネイティブは C ABI 直結**: 旧 NewDOM の方式（newdom.h）。WIT と C ABI の二重管理が生じる。
- **WIT 単一ソース・ターゲット別コンパイル（採用）**: 仕様の管理点が一つ。Web はブラウザ Wasm ランタイムを使い、ネイティブは Wasm ランタイム不要。wit-bindgen が差を吸収する。

## Consequences

- cbindgen + newdom.h による C ABI の**独立メンテナンスを廃止**する。C ユーザーは引き続き C ヘッダーで使えるが、そのヘッダーは cbindgen ではなく wit-bindgen-c が WIT から生成する
- WIT が唯一の公開インターフェース定義ソースとなり、C ヘッダー・Rust crate・Wasm コンポーネント等がすべてそこから派生する（WIT が C ABI をラップするのではなく、WIT から C ABI が派生する）
- 言語 SDK は wit-bindgen の各言語プラグインから自動生成される（Rust / C / Zig / Go / AssemblyScript 等）
- ネイティブビルドのバイナリに Wasm ランタイムが含まれない
