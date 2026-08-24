# Houou（鳳凰）— 自作将棋エンジン

`Tsubame/examples/shogi-demo` の AI を置き換えるための将棋エンジン。
Web（wasm）で動いており、ネイティブ（Torimi）は Phase 5。

## いまどこまで出来ているか

**Phase 0（盤・合法手生成）／ Phase 1（探索・評価・USI）／ Phase 2（wasm・demo 統合）まで完了。**
残るは Phase 3（評価の作り込み）と Phase 4（NNUE）。

```
$ cargo run --release --bin houou -- bench

平手 perft。既知の値と突き合わせながら nps を測る。
depth           nodes       time           nps
    1              30     0.000s       3303964  ok
    2             900     0.000s       6916904  ok
    3           25470     0.003s       8143796  ok
    4          719731     0.068s      10598106  ok
    5        19861490     1.384s      14347387  ok

平手からの探索。評価と枝刈りを含む実効 nps。
depth           nodes       time           nps  pv
    8          168182     0.275s        610995  7g7f 3c3d 2h5h 8b5b 8h2b+ 3a2b
   10          445138     0.732s        607565  7g7f 3c3d 2h5h 8b6b 8h2b+ 3a2b
```

置き換えた相手（内蔵 TS エンジン）との差:

| | 内蔵 TS | Houou（ネイティブ） | Houou（ブラウザ wasm） |
|---|---|---|---|
| 実効 nps | 約 1,600 | 約 610,000 | 約 173,000 |
| 1.2 秒で届く深さ | 3 | 10〜11 | 7〜8 |

内蔵が遅いのは tsshogi の合法手生成が 1 局面 0.6ms かかるため。

## なぜ Rust なのか

行き先が 2 つあり、両方に同じ `houou-core` を載せられるのが理由。

- **Web**: `houou-wasm`（wasm-bindgen）→ `shogi-demo` の `ShogiEngine` プラグイン。
  シングルスレッド wasm なので **COOP/COEP は要らない**。必要なのは `WebAssembly` と
  `Worker` の 2 つだけ。wasm は 150KB（gzip 38KB）。
- **ネイティブ**: Torimi の Android Rust ホストへ staticlib としてリンクし、
  埋め込み Hermes へホスト注入グローバルとして生やす。
  Hermes には `WebAssembly` も `Worker` も無く `fetch` も塞がれているので、
  **ネイティブで強いエンジンを動かせる経路はこれしかない**。

そのため `houou-core` は **依存ゼロ**を保つ。`[dependencies]` を増やす前に、
上の 2 つの行き先で足枷にならないか考えること。

## 構成

| 場所 | 役割 | 状態 |
|---|---|---|
| `crates/houou-core` | 盤・合法手生成・探索・評価 | 手作り評価まで |
| `crates/houou-cli` | USI サーバ / perft / bench / search | 動作 |
| `crates/houou-wasm` | wasm-bindgen 層 | 動作 |
| `crates/houou-nnue` | NNUE 推論 | 未着手（Phase 4） |
| `trainer/` | PyTorch で NNUE を訓練 | 未着手（Phase 4） |

## 使い方

```sh
cargo test                          # 単体テスト（perft は深さ 3 まで）
cargo test --release -- --ignored   # 深さ 5 までの perft
cargo run --release --bin houou -- bench
cargo run --release --bin houou -- search 10
cargo run --release --bin houou -- divide 3 "<SFEN>"
```

**速度を測るときは必ず `--release`。** debug は 30 倍以上遅く、測っても意味が無い。

### 将棋所 / ShogiHome から使う

エンジン登録の実行ファイルに `target/release/houou` を指定する（引数なしで USI モード）。
`USI_Hash`（MB）だけオプションを持つ。

### Web（shogi-demo）で使う

wasm パッケージを作ってから demo を動かす。**生成物は git 管理外**なので、
clone 直後は必ずこれを回すこと。

```sh
./scripts/build-wasm.sh
pnpm --filter @tsubame/example-shogi-demo dev
```

`selectEngine()` が実行時に Houou を選ぶ。`WebAssembly` か `Worker` が無ければ
内蔵 TS エンジンへ落ちるので、埋め込み Hermes でも壊れない。

## 合法手生成の正しさをどう担保するか

**2 本立てで、両方立っている。**

1. **perft** — 平手の深さ 1〜5 が公開値と一致（19,861,490 まで）。
   ただし平手から 5 手では**打ち歩詰めと行き所のない駒がほぼ出現しない**。
2. **tsshogi との差分テスト** — `shogi-demo` の
   `src/ai/houou/legal-moves-parity.test.ts`。乱数自己対局 200 局・10,000 局面超で
   houou と tsshogi の合法手集合が完全一致することを確かめる。
   加えて二歩・打ち歩詰め・行き所のない駒・王手回避を名指しで押さえる。

`shogi-demo` は「ルールの正本は tsshogi」と決めている。Houou の生成器は探索内部の
私物なので、この差分テストが 2 つを縫い合わせる橋になる。**これが落ちたら
Houou を信用してはいけない。**

perft の期待値（`perft.rs` の `HIRATE_PERFT`）を書き換えて通すのは禁止。
食い違ったら直すのは生成器のほう。

## 設計メモ

- **ビットボードは `u128`**。81 マスは 64 ビットに入らないが、Rust は `u128` を
  言語として持っており wasm32 でも i64 ペアに落ちるので、64+64 のレーン分割が要らない。
- **マス添字は筋優先**（`(筋-1)*9 + (段-1)`）。1 つの筋が連続 9 ビットになるので、
  二歩の判定が `Bitboard::file()` 1 本で済む。
- **8 方向の並びは、奇数添字が「添字が増える向き」**。飛び駒の遮蔽駒を
  lsb で取るか msb で取るかが `dir & 1` で決まる。
- **飛び駒は magic ではなくレイ方式**。magic は表が数百 KB〜MB になり wasm の
  初回ロードに効く。`bench` が movegen を律速だと言い出したら Qugiy 方式に
  差し替える — 差し替えは `attacks.rs` の中だけで閉じる。
- **合法性は盤を進めずに見る**（`Position::leaves_king_in_check`）。`do_move` /
  `undo_move` の往復を避けたことで perft が 8.2M → 11.7M nps に上がった。
  ピンの事前計算による更なる高速化は Phase 3。
- **王手判定は `do_move` が 1 回だけ計算して `StateInfo` に置く**。探索は毎ノード
  これを欲しがるので、その場で計算し直すより安い。
- **時計は差し替え可能**（`Searcher::set_clock`）。wasm32 に `std::time::Instant`
  は無く、呼ぶと panic する。wasm 側は `Date.now()` を渡している。
- **wasm の探索は途中で譲らない**ので Worker の中で回す。`engine.ts` の
  「非同期エンジンは `poll` で現在の状態だけを返す」契約にそのまま乗る。

## ライセンス

Apache-2.0。**やねうら王・Apery・tanuki- のコードも学習済み評価ファイルも
一切持ち込まない。** 持ち込んだ瞬間 GPL-3.0 が Torimi 本体まで届く。
NNUE の教師局面を自前生成するのもこの理由による。
