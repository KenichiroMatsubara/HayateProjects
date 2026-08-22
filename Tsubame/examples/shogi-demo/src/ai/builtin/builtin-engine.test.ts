import { describe, expect, it } from 'vitest';
import { Position } from 'tsshogi';
import { builtinEnginePlugin } from './builtin-engine.js';
import { listLegalMoves } from '../../rules/game.js';

const INITIAL = 'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1';

describe('内蔵エンジン', () => {
  it('初期局面から合法手を返す', async () => {
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({ sfen: INITIAL, moves: [], budgetMs: 60_000, maxDepth: 2 });
    let progress = search.poll(50);
    while (progress.kind !== 'done') progress = search.poll(50);

    expect(progress.bestMove).not.toBeNull();
    const legal = listLegalMoves(Position.newBySFEN(INITIAL)!).map((move) => move.usi);
    expect(legal, '返す手は必ず合法手').toContain(progress.bestMove);
    expect(progress.nodes).toBeGreaterThan(0);
  });

  it('詰み1手を見つける', async () => {
    // 後手玉 5一。先手は金を持ち、5二 に打てば頭金で詰み。
    // 5三 に先手の歩を置いて 4二/6二 への逃げ道をふさぐ形にする。
    const sfen = '4k4/9/4P4/9/9/9/9/9/4K4 b G 1';
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({ sfen, moves: [], budgetMs: 60_000, maxDepth: 2 });
    let progress = search.poll(100);
    while (progress.kind !== 'done') progress = search.poll(100);

    expect(progress.bestMove, '頭金で詰ます').toBe('G*5b');
  });

  it('ただ取りできる駒を取る', async () => {
    // 先手の飛車 2八、後手の角が 2六 にただで置かれている（同じ 2 筋で素抜ける）。
    const sfen = '4k4/9/9/9/9/7b1/9/7R1/4K4 b - 1';
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({ sfen, moves: [], budgetMs: 60_000, maxDepth: 2 });
    let progress = search.poll(100);
    while (progress.kind !== 'done') progress = search.poll(100);

    expect(progress.bestMove, '2六の角を飛車で取る').toBe('2h2f');
  });

  it('poll はスライス予算を守り、少しずつ進む', async () => {
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({ sfen: INITIAL, moves: [], budgetMs: 60_000, maxDepth: 3 });

    const before = Date.now();
    const progress = search.poll(5);
    const elapsed = Date.now() - before;

    // 1 局面の生成に 0.6ms ほど掛かるので、予算 5ms なら数十 ms も掛からない。
    expect(elapsed, `slice took ${elapsed}ms`).toBeLessThan(60);
    if (progress.kind === 'thinking') expect(progress.nodes).toBeGreaterThan(0);
  });

  it('cancel すると次の poll で完了になり、その時点の最善手が残る', async () => {
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({ sfen: INITIAL, moves: [], budgetMs: 60_000, maxDepth: 6 });
    search.poll(20);
    search.cancel();

    const after = search.poll(20);
    expect(after.kind).toBe('done');
    if (after.kind === 'done') expect(after.bestMove).not.toBeNull();
  });

  it('USI の指し手列を適用してから考える', async () => {
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({
      sfen: INITIAL,
      moves: ['7g7f'],
      budgetMs: 60_000,
      maxDepth: 1,
    });
    let progress = search.poll(100);
    while (progress.kind !== 'done') progress = search.poll(100);

    // 7六歩の後は後手番なので、返る手は後手の合法手。
    const position = Position.newBySFEN(INITIAL)!;
    position.doMove(position.createMoveByUSI('7g7f')!);
    expect(listLegalMoves(position).map((m) => m.usi)).toContain(progress.bestMove);
  });

  it('合法手が無い局面では bestMove が null', async () => {
    // 先手玉 5九 が詰んでいる局面（後手の金 5八・飛 5一）。
    const sfen = '4r4/9/9/9/9/9/9/4g4/4K4 b - 1';
    const engine = await builtinEnginePlugin.create();
    const search = engine.search({ sfen, moves: [], budgetMs: 60_000, maxDepth: 1 });
    let progress = search.poll(100);
    while (progress.kind !== 'done') progress = search.poll(100);
    expect(progress.bestMove).toBeNull();
  });
});
