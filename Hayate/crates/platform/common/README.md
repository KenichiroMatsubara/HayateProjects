# `platform/common/` — 全 platform 共通 capability の置き場（枠・ADR-0117）

grouping doctrine 三段階の最上段、**全 platform 共通段**の枠。すべての platform（web / mobile /
desktop）で**同一の振る舞い**として供給できる capability 実装の置き場である。現時点では昇格済みの
common capability が無いため、**ディレクトリ（枠）だけ**を置き、crate 化も capability trait の
定義もしない。

grouping doctrine の正本は [`../README.md`](../README.md)。本ファイルはその枠マーカーである。

## 枠の規律（今やらないこと）

- **capability trait を先置きしない。** 契約（trait）の正本は常に **Core**（`ImeBridge` /
  `Surface` / `FontFetcher` と同型・ADR-0068/0069）。`platform/common/` は契約の正本ではなく、
  全 platform 共通 capability の**実装**の置き場である。
- **昇格は原則 2 実装が揃ってから。** 共通 API へ持ち上げるのは、独立した 2 実装で variation が
  確認できてから（ADR-0068 の投機 seam 戒め）。1 実装だけで「共通」と決めて common へ置かない。
- **leaf 固有・family 共通と混同しない。** OS ごとに振る舞いが割れる capability は leaf
  （`platform/web` ・ `platform/mobile/<os>`）か family（`platform/mobile` ・ `platform/desktop`）
  に属する。common はあくまで全 platform で同型に供給できるものだけ。
