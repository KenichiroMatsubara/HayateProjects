# 主要依存を workspace 内にベンダリングし、upstream から自律する

Vello・Taffy・parley・fontique・skrifa を `crates/vendor/` として workspace に取り込み、upstream との依存関係を切断する。upstream の変更に引きずられず、Hayate の都合で任意のタイミングに upstream の改良を選択的に取り込む。

## 対象

| crate | 理由 |
|---|---|
| vello | 2D レンダラーの核心。API 変更が Hayate の描画パイプラインに直撃する |
| taffy | レイアウト計算の核心。独自最適化の余地がある |
| parley / fontique / skrifa | テキストスタックの核心。Linebender upstream と足並みを合わせる必要がない |

wgpu は対象外。GPU API 抽象として巨大すぎ、プラットフォーム対応の追従コストが高い。wgpu は Cargo.toml 依存として維持し、メジャーバージョンで評価して移行する。

## Considered Options

- **Cargo.toml 依存として使い続ける**: upstream の破壊的変更に強制追従させられる。substrate の安定性が外部に依存する。
- **ベンダリング（採用）**: Hayate が crate の所有者になる。upstream の bugfix は git の cherry-pick 等で任意に取り込む。

## Consequences

upstream から cherry-pick する運用が必要。ただし「取り込むかどうか」の判断権は常に Hayate 側にある。
