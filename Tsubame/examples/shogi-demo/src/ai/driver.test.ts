import { readFileSync } from 'node:fs';
import { describe, expect, it, vi } from 'vitest';
import { runEngineSearch, SLICE_BUDGET_MS } from './driver.js';
import { builtinEnginePlugin } from './builtin/builtin-engine.js';
import type { EngineSearch, EngineSearchRequest, ShogiEngine } from './engine.js';

const INITIAL = 'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1';

/**
 * ドライバの**譲り方**を守るテスト。ここが崩れると埋め込み Hermes（ネイティブホスト）で
 * 端末がハングするが、ブラウザでは症状が出ないので、ソースの形で縛る
 * （`torimi-host-fw-agnostic.test.ts` と同じ発想）。
 */
describe('runEngineSearch — Hermes ハング防止の不変条件', () => {
  const source = readFileSync(new URL('./driver.ts', import.meta.url), 'utf8');
  /** コメントを除いた実コード（注記で説明のために書いた語を拾わないため）。 */
  const code = source
    .replace(/\/\*[\s\S]*?\*\//g, '')
    .split('\n')
    .filter((line) => !line.trim().startsWith('//'))
    .join('\n');

  it('譲りは setTimeout で行う', () => {
    expect(code).toContain('setTimeout(');
  });

  it('queueMicrotask で自己連鎖しない（1フレーム内で無限に回る）', () => {
    expect(code).not.toContain('queueMicrotask');
  });

  it('Promise.then / await で自己連鎖しない', () => {
    expect(code).not.toMatch(/Promise\s*\.\s*resolve/);
    expect(code).not.toContain('.then(');
  });

  it('時計は Date.now（Hermes に performance は無い）', () => {
    expect(code).toContain('Date.now()');
    expect(code).not.toContain('performance.now');
  });

  it('requestAnimationFrame には依存しない（ネイティブでは no-op）', () => {
    expect(code).not.toContain('requestAnimationFrame');
  });
});

describe('runEngineSearch — 駆動', () => {
  it('思考を最後まで進めて最善手を返す', async () => {
    const engine = await builtinEnginePlugin.create();
    const done = await new Promise<string | null>((resolve) => {
      runEngineSearch(
        engine,
        { sfen: INITIAL, moves: [], budgetMs: 5_000, maxDepth: 1 },
        { onDone: (result) => resolve(result.bestMove) },
      );
    });
    expect(done).not.toBeNull();
  });

  it('1スライスごとに譲るので、poll は複数回に分かれる', async () => {
    const engine = await builtinEnginePlugin.create();
    const polls: number[] = [];
    const spy = spyEngine(engine, (budget) => polls.push(budget));

    await new Promise<void>((resolve) => {
      runEngineSearch(
        spy,
        { sfen: INITIAL, moves: [], budgetMs: 5_000, maxDepth: 2 },
        { onDone: () => resolve() },
      );
    });

    expect(polls.length, '1回で終わらせず刻んでいる').toBeGreaterThan(1);
    expect(new Set(polls), 'スライス予算は一定').toEqual(new Set([SLICE_BUDGET_MS]));
  });

  it('時間切れで打ち切っても、その時点の最善手を返す', async () => {
    const engine = await builtinEnginePlugin.create();
    const result = await new Promise<{ bestMove: string | null; depth: number }>((resolve) => {
      runEngineSearch(
        engine,
        // 深く掘らせつつ予算を短く切る。
        { sfen: INITIAL, moves: [], budgetMs: 30, maxDepth: 8 },
        { onDone: (done) => resolve({ bestMove: done.bestMove, depth: done.depth }) },
      );
    });
    expect(result.bestMove, '浅くても手は返す').not.toBeNull();
  });

  it('cancel すると onDone は呼ばれない', async () => {
    const engine = await builtinEnginePlugin.create();
    const onDone = vi.fn();
    const running = runEngineSearch(
      engine,
      { sfen: INITIAL, moves: [], budgetMs: 5_000, maxDepth: 4 },
      { onDone },
    );
    running.cancel();
    await new Promise((resolve) => setTimeout(resolve, 80));
    expect(onDone).not.toHaveBeenCalled();
  });

  it('進捗は深さが変わったときだけ通知する（毎スライス上げない）', async () => {
    const engine = await builtinEnginePlugin.create();
    const depths: number[] = [];
    await new Promise<void>((resolve) => {
      runEngineSearch(
        engine,
        { sfen: INITIAL, moves: [], budgetMs: 10_000, maxDepth: 2 },
        {
          onProgress: (progress) => depths.push(progress.depth),
          onDone: () => resolve(),
        },
      );
    });
    expect(new Set(depths).size, '同じ深さを二度通知しない').toBe(depths.length);
  });
});

/** `poll` の呼ばれ方だけを覗く薄い包み。 */
function spyEngine(engine: ShogiEngine, onPoll: (budget: number) => void): ShogiEngine {
  return {
    id: engine.id,
    displayName: engine.displayName,
    dispose: () => engine.dispose(),
    search(request: EngineSearchRequest): EngineSearch {
      const inner = engine.search(request);
      return {
        poll: (budget) => {
          onPoll(budget);
          return inner.poll(budget);
        },
        cancel: () => inner.cancel(),
      };
    },
  };
}
