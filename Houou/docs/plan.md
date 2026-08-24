# 自作将棋エンジン計画 — Rust → wasm、NNUE 到達まで

対象: `Tsubame/examples/shogi-demo` の AI 差し替え。Web 優先、ネイティブ(Torimi)は Phase 5 に分離。

## 0. 出発点と到達点

**現状**（実測値ではなくコードの宣言から）:
- `builtin/search.ts` — tsshogi の合法手生成が 1 局面 **0.6ms** → 約 **1,600 nps**
- `builtin-engine.ts:20` — `DEFAULT_MAX_DEPTH = 3`
- `use-engine.ts:14` — 1 手 1,200ms

**到達点**: NNUE 評価を積んだ自作エンジンで**明確に有段者**。
wasm(simd128) シングルスレッドで **20〜50万 nps** が現実的な着地で、現状比 **150〜300倍**。
1.2 秒予算なら枝刈り込みで深さ 12〜16。

## 1. 置き場所と crate 構成

新規の独立 Cargo workspace を切る。Hayate のワークスペースには入れない（vendored patch 群と無関係で、
依存ゼロに保ちたいため）。名前は仮に `Houou`（鳳凰 — 鳥の名前の系譜に乗り、中将棋の駒でもある）。好みで変更可。

```
Houou/
  Cargo.toml                 # [workspace]
  crates/
    houou-core/              # 盤・movegen・探索・手作り評価。依存ゼロ、no_std 可
    houou-nnue/              # NNUE 推論（wasm simd128 / native SIMD）
    houou-wasm/              # wasm-bindgen 層 → wasm-pkgs/houou
    houou-cli/               # native bin: USI サーバ / perft / bench / 自己対局 / 教師生成
  trainer/                   # PyTorch（NNUE 訓練）
  wasm-pkgs/houou/           # pnpm workspace package（ビルド生成物）
  docs/adr/
```

`houou-core` を**依存ゼロ**に保つのが要点。これが Phase 5 で Torimi の Android Rust ホストに
staticlib として素直にリンクできる条件になる。

pnpm 側は `pnpm-workspace.yaml` に `Houou/wasm-pkgs/houou` を追加。
wasm ビルドは `Hayate/scripts/build-wasm.sh` と同方針（wasm-opt を回さない、
`package.json` を固定生成）で `Houou/scripts/build-wasm.sh` を別に持つ。
`Hayate/scripts/wasm-build-manifest.json` は crateDir 単一前提なので、共有せず複製する。

## 2. ライセンスと tsshogi の位置づけ（先に決めておく）

- **やねうら王 / Apery / tanuki- のコードも学習済み評価ファイルも一切持ち込まない。**
  持ち込んだ瞬間 GPL-3.0 が Torimi 本体まで届く。参考にするのは公開文献のアルゴリズムまで。
  自作なので全体を Apache-2.0（リポジトリの既定）で統一できる。
- **対局ルールの正本は tsshogi のまま**。`README` の「二重管理を避ける」原則は
  「合法性の最終判断は 1 箇所」として保つ。エンジンの movegen は**探索内部の私物**で、
  返した bestmove は必ず `position.createMoveByUSI` → `isValidMove` を通してから盤に載せる。
  両者の橋渡しが Phase 0 の差分テスト。

---

## Phase 0 — 盤・movegen・測定基盤（1〜2週）

強さは変わらない。ここでの正しさが後段すべての土台になる。

**houou-core**
- 盤表現: 81 マス = **`u128` bitboard**。Rust の u128 は wasm32 では i64 ペアに落ちるが問題ない。
- 飛び駒の利き: **Qugiy 方式**（128bit の加減算とバイト反転で飛車・角の利きを出す）。
  magic bitboard より**テーブルが小さい**ので、wasm の初回ロードに効く。
- 持ち駒: packed u32。
- Zobrist hash（手番・持ち駒込み）。置換表と千日手検出の両方で使う。
- movegen: 全合法手。二歩・打ち歩詰め・行き所のない駒・王手放置・連続王手の千日手まで。
- `do_move` / `undo_move` は差分（後で NNUE accumulator が同じ経路に乗る）。

**測定基盤（先に書く）**
- `houou-cli perft` — 平手初期局面の公開値と突き合わせる:
  `1:30 / 2:900 / 3:25,470 / 4:719,731 / 5:19,861,490`
- **tsshogi 差分テスト** — 自己対局から抽出した 10 万局面で、houou の合法手集合(USI 文字列)と
  tsshogi の `isValidMove` 由来の集合を照合する Node 側 harness。
- `houou-cli bench` — nps 計測。

**完了条件**: perft(5) 一致 / tsshogi 差分ゼロ / movegen-only で native 5M nps 以上 / perft を CI に。

## Phase 1 — 探索の骨格＋現行評価の移植（2〜3週）

**この Phase の終わりで既に現行 AI を桁で置き去りにする。**

- 反復深化 + PVS（αβ）
- 置換表（可変サイズ。wasm では 16〜64MB）
- move ordering: TT手 → 捕獲(SEE / MVV-LVA) → killer → history
- 静止探索（捕獲と王手回避）
- null move pruning / LMR / futility / delta pruning
- 詰みスコアの深さ割引、探索内の千日手検出
- 評価は現行 `evaluate.ts`（駒割＋前進＋玉の安全）を Rust に写すだけ。差分更新できる形に整えておく。
- **USI プロトコルを喋る `houou-cli`** — 将棋所 / ShogiHome から対局でき、エンジン同士を自動対戦できる。
  強さの測定土台なのでここで要る。後回しにしない。

**完了条件**: 現 builtin に 100 局で 99% 以上勝つ / ShogiHome から USI で動く。

## Phase 2 — wasm 化して demo に載せる（1週）

**既存の `ShogiEngine` シームを一切変えない。**

- `houou-wasm` の API を `engine.ts` の語彙にそのまま合わせる:
  `new_search(sfen, moves, budget_ms)` / `poll(slice_ms) -> {depth,nodes,score_cp,best_move?}` / `cancel()`
- ビルドフラグ: `-C target-feature=+simd128`。
  **シングルスレッドなので COOP/COEP は不要**。`strong/capability.ts` が塞いでいたのは
  `-pthread` で shared memory 宣言された既製 wasm の話で、自作のシングルスレッド版には当たらない。
  必要なのは `WebAssembly` の存在だけ。
- 実行場所は **Worker 内**が第一候補（フレームを一切食わない）。`engine.ts` が
  「非同期エンジンは poll で現在値だけ返す」と既に定義しているので契約に乗る。
  Worker が無い環境向けにメインスレッド時間刻み版をフォールバックで持つ。
- `select-engine.ts` の `PLUGINS` 先頭に `hououEnginePlugin` を追加。
  `isAvailable()` = `hasWebAssembly()`。生成失敗時に builtin へ落ちる既存動作がそのまま安全網。
- wasm バイナリは初回 fetch → Cache API。
- CI: `wasm-c3.yml` に倣って houou の wasm ビルド + 既存 e2e（`ai-turn.spec.ts`）。

**完了条件**: `pnpm dev` で houou が選ばれ、1.2 秒予算で深さ 10 以上、フレーム落ちなし。

## Phase 3 — 手作り評価の上限まで（2〜4週）

- 駒-位置表(PST)、利きの数、駒の連結、玉周りの危険度（攻め駒数 × 利き）、
  飛車先/香車の突破、成りの価値
- 差分更新（`do_move`/`undo_move` に相乗り）
- **チューニング手段が本体**: 自己対局 SPRT（逐次検定）。USI 同士を戦わせる対局マネージャを
  `houou-cli` に持たせる（200行程度）。感覚で係数をいじるのを禁じる。

**完了条件**: Phase 1 に対し +200 Elo 以上。体感でアマ級位者〜初段手前。

## Phase 4 — NNUE（1〜3ヶ月。ここが本体）

3 つのサブプロジェクトに割れる。計算時間が支配的。

### 4a. 教師局面の生成
- Phase 3 のエンジンで自己対局（固定深さ 6〜8 or 固定ノード）→
  局面 + 探索評価値 + 最終勝敗 を吐く。フォーマットは自作（packed sfen 32byte + eval + result）。
- ネイティブ multi-thread で回す。目安 **1 億局面**、この機械で数日〜。
- 序盤の多様性は floodgate（wdoor）の公開棋譜を初期局面プールに混ぜると立ち上がりが早い。
- **自前生成なので学習済み評価のライセンス問題が発生しない。ここが GPL 回避の要。**

### 4b. 特徴量とネット形状 — Web の配信量が制約になる
HalfKP は 玉81 × 駒-位置1548 = **125,388 特徴**。特徴変換器の重みが `dims × 125,388 × 2byte` で効く:

| dims | 特徴変換器サイズ | 判定 |
|---|---|---|
| 256（標準） | 約 64MB | Web 配信は厳しい |
| **128** | **約 32MB**（brotli で 20MB 台） | **第一候補** |
| 64 | 約 16MB | 弱くなるが軽い |

- **HalfKP 128×2-32-32 を第一候補**。初回だけ落として Cache API / IndexedDB に置く。
  もっと絞るなら玉のバケット化で特徴数自体を減らす。
- 量子化: 特徴変換器 int16 / 隠れ層 int8。
- 推論は wasm simd128（`i16x8` / `i32x4` dot）。**accumulator の差分更新が必須**
  — Phase 0 で `do_move` を差分にしておいたのがここで効く。

### 4c. 訓練
- `trainer/` に PyTorch。損失は評価値の回帰 + 勝敗のクロスエントロピー混合（λ で絞る）。
- ループ: 生成 → 訓練 → SPRT で強さ検証 → 新エンジンで再生成、を 3〜5 世代。
- GPU が要る。手元に無ければ Colab / Vast.ai。

**完了条件**: Phase 3 に対し +300 Elo 以上。floodgate に投げてレートを実測。

## Phase 5 —（後日）ネイティブ Torimi（1週）

- `houou-core` は依存ゼロなので、Torimi の Android Rust ホストに **staticlib としてリンク**し、
  Hermes へ `__houou.search()` 相当のホスト注入グローバルとして生やす。
  JS 側は `nativeHououPlugin` を `PLUGINS` に足すだけ。
- **これが Rust を選んだ主な理由**: Hermes には `WebAssembly` も `Worker` も無く、
  `native-prelude.ts:88` で `fetch` すら reject スタブなので、
  JS 側からは wasm もサーバも到達できない。Rust をホストに載せる経路だけが空いている。
- 探索は別スレッドで走らせ、`pump_frame` のたびに結果をポーリング（`driver.ts` の作法に合わせる）。
- NNUE ファイルは APK 同梱（自作なので配布自由）。

---

## 横断的に決めておくこと

- **CI に perft 回帰**を必ず入れる。movegen のデグレは後段すべてを腐らせ、しかも静かに壊れる。
- **強さの測定は SPRT のみ**。1 局や体感で判断しない。
- nps / perft / Elo の 3 つを毎 Phase の完了条件に置く。

## 見積まとめ

| Phase | 内容 | 期間 | 到達 |
|---|---|---|---|
| 0 | 盤・movegen・perft・差分テスト | 1〜2週 | 強さ変化なし |
| 1 | 探索骨格 + 現行評価移植 + USI | 2〜3週 | 現行を桁で超える |
| 2 | wasm 化・demo 統合 | 1週 | **Web で実際に強い** |
| 3 | 手作り評価の作り込み | 2〜4週 | アマ級位〜初段手前 |
| 4 | NNUE（生成・訓練・推論） | 1〜3ヶ月 | **明確に有段者** |
| 5 | ネイティブ Torimi | 1週 | Android でも同じ強さ |

Phase 2 の終わりで実用上の満足はほぼ得られる。Phase 4 は計算資源と根気の勝負。
