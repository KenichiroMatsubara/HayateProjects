import { describe, expect, it } from 'vitest';
import { createTsubameRoot } from '@torimi/tsubame-react';
import { RecordingRenderer, type RecordedCall } from '@torimi/tsubame-renderer-protocol';
import { Color, PieceType, Position, Square } from 'tsshogi';
import { Board, type BoardViewState } from './Board.js';
import { Hand } from './Hand.js';
import { CellPainter } from '../paint/cell-painter.js';
import { HandSlotPainter } from '../paint/hand-painter.js';
import { CSS } from '../paint/palette.js';

/**
 * 盤と駒台の**構造**の検証。
 *
 * 「木地・罫線・ハイライトは要素、駒だけが draw」という切り分けは painter の単体
 * テストからは見えない（painter は自分が描かないものについて何も言えない）。ここでは
 * Renderer Protocol の呼び出し列そのものを読んで、絵にすべきでないものが絵に
 * なっていないことを主張する。
 */

const INITIAL = 'lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1';

function boardState(overrides: Partial<BoardViewState> = {}): BoardViewState {
  return {
    position: Position.newBySFEN(INITIAL)!,
    selected: -1,
    targets: [],
    lastFrom: -1,
    lastTo: -1,
    checkedKing: -1,
    flipped: false,
    ...overrides,
  };
}

function render(node: React.ReactElement): RecordedCall[] {
  const renderer = new RecordingRenderer();
  createTsubameRoot(renderer).render(node);
  return renderer.calls;
}

/** style 呼び出しを要素 id ごとにまとめる（同じ要素への patch は後勝ちで合成）。 */
function styles(calls: readonly RecordedCall[]): Map<number, Record<string, unknown>> {
  const merged = new Map<number, Record<string, unknown>>();
  for (const call of calls) {
    if (call.method !== 'setStyle') continue;
    const id = call.id as unknown as number;
    merged.set(id, { ...(merged.get(id) ?? {}), ...call.style });
  }
  return merged;
}

function draws(calls: readonly RecordedCall[]): Extract<RecordedCall, { method: 'setDraw' }>[] {
  return calls.filter((c) => c.method === 'setDraw') as Extract<
    RecordedCall,
    { method: 'setDraw' }
  >[];
}

describe('盤の構造 — 絵ではなく要素で組む', () => {
  it('木地・罫線・外枠は要素の style が持つ', () => {
    const all = [...styles(render(<Board state={boardState()} onTapSquare={() => {}} />)).values()];

    const board = all.find((s) => s.display === 'grid');
    expect(board, '盤は 9×9 の grid 要素').toBeDefined();
    // 罫線は線を引くのではなく、枡の隙間から盤の背景を覗かせて作る。
    expect(board!.gap).toBe(1);
    expect(board!.backgroundColor).toBe(CSS.gridLine);
    expect(board!.borderColor).toBe(CSS.boardEdge);

    const wood = all.filter((s) => s.backgroundColor === CSS.wood);
    expect(wood, '81 枡すべてが木地を背景として持つ').toHaveLength(81);
  });

  it('駒のある枡だけが draw を持ち、空き枡は null で消す', () => {
    const calls = render(<Board state={boardState()} onTapSquare={() => {}} />);
    const painted = draws(calls).filter((c) => c.value !== null);
    const cleared = draws(calls).filter((c) => c.value === null);

    // 初期局面の駒は 40 枚。星は 4 交点だが、そのうち 2 つは歩の枡と重なる。
    expect(painted).toHaveLength(42);
    expect(painted.every((c) => c.value instanceof CellPainter)).toBe(true);
    expect(cleared.length, '残りの枡は display list を持たない').toBe(81 - 42);
  });

  it('選択と直前手は背景色で表す — 絵は 1 ドットも変わらない', () => {
    const plain = render(<Board state={boardState()} onTapSquare={() => {}} />);
    const marked = render(
      <Board
        state={boardState({ selected: new Square(7, 7).index, lastTo: new Square(7, 6).index })}
        onTapSquare={() => {}}
      />,
    );

    const tones = [...styles(marked).values()].map((s) => s.backgroundColor);
    expect(tones).toContain(CSS.woodSelected);
    expect(tones).toContain(CSS.woodLastMove);

    // ハイライトは要素の背景なので、描かれる枡の数は変わらない。
    expect(draws(marked).filter((c) => c.value !== null)).toHaveLength(
      draws(plain).filter((c) => c.value !== null).length,
    );
  });

  it('81 枡すべてがタップを受ける（当たり判定は要素のまま）', () => {
    const calls = render(<Board state={boardState()} onTapSquare={() => {}} />);
    expect(calls.filter((c) => c.method === 'addEventListener')).toHaveLength(81);
  });
});

describe('駒台の構造', () => {
  const withHand = Position.newBySFEN(
    'lnsgkgsnl/1r5b1/pppppppp1/9/9/9/PPPPPPPP1/1B5R1/LNSGKGSNL b 2PS 1',
  )!;

  function renderHand(selected: PieceType | null = null): RecordedCall[] {
    return render(
      <Hand
        color={Color.BLACK}
        hand={withHand.hand(Color.BLACK)}
        active
        selected={selected}
        flipped={false}
        onTapPiece={() => {}}
      />,
    );
  }

  it('枠ごとに駒 1 枚を描く', () => {
    const painted = draws(renderHand()).filter((c) => c.value !== null);
    // 歩 2 枚・銀 1 枚 → 枠は 2 つ。
    expect(painted).toHaveLength(2);
    expect(painted.every((c) => c.value instanceof HandSlotPainter)).toBe(true);
  });

  it('枚数は text 要素で出す（2 枚以上のときだけ）', () => {
    const texts = renderHand()
      .filter((c) => c.method === 'setText')
      .map((c) => (c as Extract<RecordedCall, { method: 'setText' }>).text);
    // 歩が 2 枚、銀は 1 枚。ラベルの「☗先手」以外に出る数字は "2" だけ。
    expect(texts).toContain('2');
    expect(texts.filter((t) => /^\d+$/.test(t))).toEqual(['2']);
  });

  it('選択の座布団は枠の背景色', () => {
    const tones = [...styles(renderHand(PieceType.PAWN)).values()].map((s) => s.backgroundColor);
    expect(tones).toContain(CSS.handSelected);
  });
});
