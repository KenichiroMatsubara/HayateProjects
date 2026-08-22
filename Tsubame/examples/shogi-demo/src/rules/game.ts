import {
  Color,
  type ImmutablePosition,
  Move,
  PieceType,
  Record,
  Square,
  SpecialMoveType,
  handPieceTypes,
  isPromotable,
  reverseColor,
  specialMove,
} from 'tsshogi';

/**
 * 対局の状態機械。ルールの正否は **tsshogi が正本**で、ここはアプリの語彙
 * （手番・結果・待った・合法手の一覧）へ翻訳するだけの薄い層に保つ。
 * 禁じ手（二歩・打ち歩詰め・行き所のない駒・王手放置）の判定を自前で持たないのは、
 * 二重管理を避けるため（`Position.isValidMove` が単一の判断点）。
 */

/** 対局の決着。`null` は続行中。 */
export type GameResult =
  | { readonly kind: 'checkmate'; readonly winner: Color }
  | { readonly kind: 'resign'; readonly winner: Color }
  | { readonly kind: 'repetition-draw' }
  | { readonly kind: 'perpetual-check'; readonly winner: Color };

/** 同一局面がこの回数現れたら千日手（現局面を含む）。 */
const REPETITION_LIMIT = 4;

/**
 * 盤上の駒を動かす手と、駒台から打つ手を同じ形で表す UI 側の語彙。
 * `from` が `Square` なら移動、`PieceType` なら打ち（tsshogi の `createMove` と同じ約束）。
 */
export type MoveSource = Square | PieceType;

export class ShogiGame {
  private readonly record: Record;
  private result: GameResult | null = null;
  private revisionCount = 0;

  /**
   * @param initialPosition 開始局面。省略すると平手。駒落ちや詰将棋の局面を
   *   そのまま渡せるようにしておく（テストもこの入口を使う）。
   */
  constructor(initialPosition?: ImmutablePosition) {
    this.record = new Record(initialPosition);
  }

  /** 現局面（読み取り専用）。 */
  get position(): ImmutablePosition {
    return this.record.position;
  }

  get sideToMove(): Color {
    return this.position.color;
  }

  /** 王手がかかっているか。 */
  get inCheck(): boolean {
    return this.position.checked;
  }

  get ply(): number {
    return this.record.current.ply;
  }

  get sfen(): string {
    return this.position.sfen;
  }

  /** 決着していれば結果、続行中なら `null`。 */
  get outcome(): GameResult | null {
    return this.result;
  }

  get isOver(): boolean {
    return this.result !== null;
  }

  /**
   * 盤・持ち駒・選択状態のいずれかが変わるたびに増える版数。painter の
   * `shouldRepaint` と React の再描画トリガに使う（局面オブジェクトは
   * その場更新されるので identity 比較が効かない）。
   */
  get revision(): number {
    return this.revisionCount;
  }

  /** 直前の指し手（ハイライト用）。特殊手・初手前は `null`。 */
  get lastMove(): Move | null {
    const move = this.record.current.move;
    return move instanceof Move ? move : null;
  }

  /**
   * 指し手を作る。合法でなければ `null`。成りの可否は呼び出し側が
   * {@link promotionChoice} で先に問い合わせる。
   */
  createMove(from: MoveSource, to: Square, promote: boolean): Move | null {
    const move = this.position.createMove(from, to);
    if (move === null) return null;
    const candidate = promote ? move.withPromote() : move;
    return this.position.isValidMove(candidate) ? candidate : null;
  }

  /**
   * `from` → `to` に対する成りの選択肢。
   * - `'forced'`: 不成が非合法（行き所のない駒）。問わずに成らせる。
   * - `'optional'`: 成・不成の両方が合法。ユーザーに選ばせる。
   * - `'none'`: 成れない。
   * - `'illegal'`: そもそも指せない。
   */
  promotionChoice(from: MoveSource, to: Square): 'forced' | 'optional' | 'none' | 'illegal' {
    const move = this.position.createMove(from, to);
    if (move === null) return 'illegal';
    const plainOk = this.position.isValidMove(move);
    if (!isPromotable(move.pieceType) || from instanceof Square === false) {
      return plainOk ? 'none' : 'illegal';
    }
    const promoteOk = this.position.isValidMove(move.withPromote());
    if (promoteOk && plainOk) return 'optional';
    if (promoteOk) return 'forced';
    return plainOk ? 'none' : 'illegal';
  }

  /** 着手する。成功したら `true`。決着後は常に `false`。 */
  play(move: Move): boolean {
    if (this.result !== null) return false;
    if (!this.record.append(move)) return false;
    this.revisionCount += 1;
    this.result = this.judge();
    return true;
  }

  /** 投了する。手番側の負け。 */
  resign(): boolean {
    if (this.result !== null) return false;
    this.record.append(specialMove(SpecialMoveType.RESIGN));
    this.revisionCount += 1;
    this.result = { kind: 'resign', winner: reverseColor(this.sideToMove) };
    return true;
  }

  /**
   * 待った。`plies` 手ぶん戻す（AI 対局では 2 手 = 自分と相手の 1 往復）。
   * 1 手も戻せなければ `false`。決着後も戻せる（投了の取り消しを含む）。
   */
  undo(plies: number): boolean {
    let undone = 0;
    while (undone < plies && this.record.goBack()) undone += 1;
    if (undone === 0) return false;
    this.record.removeNextMove();
    this.revisionCount += 1;
    this.result = this.judge();
    return true;
  }

  /**
   * 手番側の合法手をすべて列挙する。tsshogi は生成器を公開していないため、
   * 盤上の自駒 × 全マス と 持ち駒 × 全マス を `isValidMove` で漉す。
   * UI のハイライト用としては十分速い（AI の探索は同じ列挙を再利用する）。
   */
  listLegalMoves(): Move[] {
    return listLegalMoves(this.position);
  }

  /** `from` から指せる移動先の一覧（成・不成のどちらかが合法なら含める）。 */
  listTargets(from: MoveSource): Square[] {
    const targets: Square[] = [];
    for (const to of Square.all) {
      if (this.promotionChoice(from, to) !== 'illegal') targets.push(to);
    }
    return targets;
  }

  /** 千日手・連続王手・詰みを見て決着を判定する。 */
  private judge(): GameResult | null {
    const position = this.position;
    const perpetual = this.record.perpetualCheck;
    if (perpetual !== null) {
      // 連続王手の千日手は**王手を掛け続けた側**の負け。
      return { kind: 'perpetual-check', winner: reverseColor(perpetual) };
    }
    if (this.record.getRepetitionCount(position) >= REPETITION_LIMIT) {
      return { kind: 'repetition-draw' };
    }
    if (listLegalMoves(position).length === 0) {
      // 合法手が無い = 詰み（将棋にステイルメイトは無い）。
      return { kind: 'checkmate', winner: reverseColor(position.color) };
    }
    return null;
  }
}

/**
 * 手番側の合法手を列挙する。`ShogiGame` の外からも AI が使うので独立した関数にする。
 * 打ちは「持っている駒種」だけを試し、盤上は「自分の駒があるマス」だけを起点にする。
 */
export function listLegalMoves(position: ImmutablePosition): Move[] {
  const moves: Move[] = [];
  const color = position.color;

  for (const from of position.board.listNonEmptySquares()) {
    if (position.board.at(from)?.color !== color) continue;
    for (const to of Square.all) {
      const move = position.createMove(from, to);
      if (move === null) continue;
      if (position.isValidMove(move)) moves.push(move);
      if (isPromotable(move.pieceType)) {
        const promoted = move.withPromote();
        if (position.isValidMove(promoted)) moves.push(promoted);
      }
    }
  }

  const hand = position.hand(color);
  for (const pieceType of handPieceTypes) {
    if (hand.count(pieceType) === 0) continue;
    for (const to of Square.all) {
      const move = position.createMove(pieceType, to);
      if (move !== null && position.isValidMove(move)) moves.push(move);
    }
  }

  return moves;
}
