import { dirname, join } from 'node:path';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { beforeAll, describe, expect, it } from 'vitest';
import { Position } from 'tsshogi';
import { listLegalMoves } from '../../rules/game.js';

/**
 * **Houou の合法手生成が tsshogi と一致することの担保。**
 *
 * このデモは「ルールの正否は tsshogi が正本」と決めている（`README.md`）。Houou は
 * 探索を速くするために自前の生成器を持つので、そのままでは判断が 2 箇所に分かれる。
 * この差分テストがその 2 つを縫い合わせる橋で、**これが無い限り Houou の生成器は
 * 信用してはいけない**。
 *
 * perft（`Houou/crates/houou-core/src/perft.rs`）は平手からの手数が浅く、
 * 打ち歩詰めや行き所のない駒がほとんど現れない。ここは実戦的な局面を通るので、
 * perft が見ていない範囲を埋める。
 *
 * 局面は乱数の自己対局で作る。seed 固定なので、落ちたら同じ手順を再現できる。
 */

const require = createRequire(import.meta.url);

type WasmModule = typeof import('@houou/wasm');
let wasm: WasmModule;
let engine: InstanceType<WasmModule['HououEngine']>;

/** 決定的な乱数（splitmix32）。落ちたときに同じ対局を再現できるようにする。 */
function makeRandom(seed: number): () => number {
  let state = seed >>> 0;
  return () => {
    state = (state + 0x9e3779b9) >>> 0;
    let z = state;
    z = Math.imul(z ^ (z >>> 16), 0x21f0aaad) >>> 0;
    z = Math.imul(z ^ (z >>> 15), 0x735a2d97) >>> 0;
    return ((z ^ (z >>> 15)) >>> 0) / 4294967296;
  };
}

beforeAll(async () => {
  // wasm-pack の `--target web` 出力は既定で `fetch` を使う。node では
  // バイト列を直接渡す `initSync` に切り替える（repo の既存 wasm テストと同じ流儀）。
  const entry = require.resolve('@houou/wasm');
  wasm = (await import('@houou/wasm')) as WasmModule;
  wasm.initSync({ module: readFileSync(join(dirname(entry), 'houou_bg.wasm')) });
  engine = new wasm.HououEngine(1);
});

/** tsshogi が認める合法手を USI で、並び順に依らない形で返す。 */
function tsshogiMoves(sfen: string, moves: readonly string[]): string[] {
  const position = Position.newBySFEN(sfen);
  if (position === null) throw new Error(`tsshogi が読めない SFEN: ${sfen}`);
  for (const usi of moves) {
    const move = position.createMoveByUSI(usi);
    if (move === null || !position.doMove(move)) {
      throw new Error(`tsshogi が指せない手: ${usi}`);
    }
  }
  return listLegalMoves(position)
    .map((move) => move.usi)
    .sort();
}

function hououMoves(sfen: string, moves: readonly string[]): string[] {
  return [...engine.legalMoves(sfen, moves.join(' '))].sort();
}

/** 両者の食い違いを、原因を追える形の文言にする。 */
function describeDifference(mine: string[], theirs: string[]): string {
  const extra = mine.filter((m) => !theirs.includes(m));
  const missing = theirs.filter((m) => !mine.includes(m));
  return [
    extra.length > 0 ? `houou だけが挙げた手: ${extra.join(' ')}` : '',
    missing.length > 0 ? `houou が落とした手: ${missing.join(' ')}` : '',
  ]
    .filter(Boolean)
    .join(' / ');
}

const HIRATE = 'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1';

describe('Houou の合法手生成は tsshogi と一致する', () => {
  it('平手の初期局面', () => {
    expect(hououMoves(HIRATE, [])).toEqual(tsshogiMoves(HIRATE, []));
  });

  it.each([
    ['二歩', '4k4/9/9/9/9/9/4P4/9/4K4 b P 1'],
    ['と金は二歩にならない', '4k4/9/9/9/9/9/4+P4/9/4K4 b P 1'],
    ['行き所のない駒', '4k4/9/9/9/9/9/9/9/4K4 b PLN 1'],
    ['成らないと死ぬ歩', '8k/4P4/9/9/9/9/9/9/4K4 b - 1'],
    ['打ち歩詰め', '8k/9/8G/9/9/9/9/9/K6R1 b P 1'],
    ['打ち歩詰めにならない歩打ち', '8k/9/8G/9/9/9/9/9/K6R1 b 2P 1'],
    ['王手を受けている', '4k4/9/9/9/4R4/9/9/9/4K4 w - 1'],
    ['持ち駒が多い中盤', 'l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1'],
  ])('%s', (_name, sfen) => {
    const mine = hououMoves(sfen, []);
    const theirs = tsshogiMoves(sfen, []);
    expect(mine, describeDifference(mine, theirs)).toEqual(theirs);
  });

  // 律速は tsshogi 側（1 局面あたり 0.6ms 前後）。1 万局面超を通すので十数秒かかる。
  // 生成器の正しさは他のすべての土台なので、この時間は払う価値がある。
  it('乱数の自己対局 200 局・各手番で一致する', { timeout: 120_000 }, () => {
    const random = makeRandom(0x484f_554f);
    let positionsChecked = 0;

    for (let game = 0; game < 200; game += 1) {
      const played: string[] = [];

      for (let ply = 0; ply < 90; ply += 1) {
        const mine = hououMoves(HIRATE, played);
        const theirs = tsshogiMoves(HIRATE, played);

        expect(
          mine,
          `${game} 局目 ${ply} 手目（moves: ${played.join(' ')}）— ${describeDifference(mine, theirs)}`,
        ).toEqual(theirs);
        positionsChecked += 1;

        // 詰みか手詰まり。次の対局へ。
        if (mine.length === 0) break;
        played.push(mine[Math.floor(random() * mine.length)]!);
      }
    }

    // 打ち歩詰めのような稀な禁じ手に当たるだけの局面数を通っていること。
    expect(positionsChecked).toBeGreaterThan(10_000);
  });
});
