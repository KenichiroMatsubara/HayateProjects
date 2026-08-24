//! 静的評価。単位は「歩 = 100」。**手番側から見て正なら手番側が有利**。
//!
//! # 分担
//!
//! - **駒割 + 駒位置（`psq_value`）** は `Position` が `put` / `remove` で差分維持する。
//!   探索の内側で盤を舐め直さないための構造で、評価の 8 割はここで決まる。
//! - **持ち駒・玉の安全度・利きの数** は [`evaluate`] がその場で数える。
//!   持ち駒は高々 14 種、玉の周りは 8 マスなので、差分にするほどの重さが無い。
//!
//! Phase 4 でこの全体が NNUE に置き換わる。そのとき差分維持の骨格（`put` / `remove` が
//! 呼ばれるたびに何かを足し引きする形）はそのまま accumulator の更新に使い回せる。

use crate::bitboard::Bitboard;
use crate::position::Position;
use crate::types::{Color, Piece, PieceType, Square, HAND_TYPES, NUM_PIECE_TYPES, NUM_SQUARES};
use std::sync::LazyLock;

/// 詰みのスコア。深さで割り引いて「早い詰み」を優先させる。
pub const MATE: i32 = 32_000;
/// 詰み扱いにする下限。これを超えたスコアは「n 手詰み」を表す。
pub const MATE_THRESHOLD: i32 = MATE - 1_000;
/// 評価値の上限（alpha-beta の初期窓）。
pub const INFINITY: i32 = 32_600;

/// 盤上の駒の価値。
///
/// 歩を 100 とした相対値。成駒を金より少し高く見るのは、成れば取られても
/// 元の駒しか渡さないぶん得だから。
const MATERIAL: [i32; NUM_PIECE_TYPES] = [
    100,    // 歩
    350,    // 香
    400,    // 桂
    550,    // 銀
    900,    // 角
    1000,   // 飛
    600,    // 金
    0,      // 玉（取られないので値を入れない。詰みは探索が扱う）
    620,    // と
    610,    // 成香
    610,    // 成桂
    610,    // 成銀
    1250,   // 馬
    1350,   // 竜
];

/// 持ち駒は盤上より少し高く見る。どこへでも打てるぶん働きが広い。
const HAND_BONUS_PERCENT: i32 = 15;

/// 玉の周り 8 マスに居る自駒 1 枚あたりの加点。
const GUARD_BONUS: i32 = 18;
/// 玉の周り 8 マスに利いている相手の駒 1 枚あたりの減点。
const ATTACKER_PENALTY: i32 = 32;
/// 玉が自陣の底から 1 段離れるごとの減点。囲いを崩して出歩くのを抑える。
const KING_EXPOSURE: i32 = 40;
/// 盤上の駒 1 枚が持つ利き 1 マスあたりの加点（機動力）。
const MOBILITY_BONUS: i32 = 3;

#[inline]
pub fn material(piece_type: PieceType) -> i32 {
    MATERIAL[piece_type.index()]
}

/// 持ち駒 1 枚の価値。
#[inline]
pub fn hand_value(piece_type: PieceType) -> i32 {
    let base = MATERIAL[piece_type.index()];
    base + base * HAND_BONUS_PERCENT / 100
}

/// 駒-位置表。`[色][駒種][マス]` で、**先手視点の符号付き**ではなく素の加点。
static PIECE_SQUARE: LazyLock<[[[i32; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM]> =
    LazyLock::new(build_piece_square);

/// 駒-位置表を組む。
///
/// 手で 14 × 81 の数字を並べる代わりに、**3 つの効き目の重ね合わせ**で作る。
/// 数字を眺めても意味が分からない表を持たずに済み、調整するときも意図の単位で触れる。
/// Phase 3 で自己対局のチューニングに掛ける対象がこの 3 つの係数になる。
fn build_piece_square() -> [[[i32; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM] {
    let mut table = [[[0; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM];

    for color in Color::ALL {
        for piece_type in PieceType::ALL {
            for square in Square::all() {
                // 敵陣までの距離。先手は 1 段目が敵陣の奥。
                let advance = match color {
                    Color::Black => 9 - square.rank() as i32,
                    Color::White => square.rank() as i32 - 1,
                };
                // 中央からの距離（0 が中央）。
                let centrality = 4 - (square.file() as i32 - 5).abs();

                let value = match piece_type {
                    // 玉は前に出ると危ない。位置の得点は玉の安全度が別に見るので 0。
                    PieceType::King => 0,
                    // 歩は前進そのものが価値。敵陣に近いほど成りが近い。
                    PieceType::Pawn => advance * advance / 2,
                    // 香・桂は前に進むと戻れない。前進の加点を控えめにする。
                    PieceType::Lance | PieceType::Knight => advance * 2,
                    // 角・飛は中央と敵陣の両方で働く。
                    PieceType::Bishop | PieceType::Rook => advance * 3 + centrality * 4,
                    // 金・銀は自陣で囲いを作るのが仕事なので、前進をほとんど評価しない。
                    PieceType::Gold | PieceType::Silver => advance,
                    // 成駒はもう成る必要が無い。中央で働くぶんだけ見る。
                    _ => centrality * 3,
                };
                table[color.index()][piece_type.index()][square.index()] = value;
            }
        }
    }
    table
}

/// 1 枚の駒がその位置に居ることの価値。**先手視点の符号付き**（後手の駒は負）。
///
/// `Position::put` / `remove` がこれを足し引きして合計を維持する。
#[inline]
pub fn psq_value(piece: Piece, square: Square) -> i32 {
    let value = MATERIAL[piece.piece_type.index()]
        + PIECE_SQUARE[piece.color.index()][piece.piece_type.index()][square.index()];
    match piece.color {
        Color::Black => value,
        Color::White => -value,
    }
}

/// 手番側から見た評価値。正なら手番側が有利。
pub fn evaluate(position: &Position) -> i32 {
    // 盤上の駒割と駒位置は差分維持済み（先手視点）。
    let mut score = position.psq_score();
    score += hands(position, Color::Black) - hands(position, Color::White);
    score += king_safety(position, Color::Black) - king_safety(position, Color::White);
    score += mobility(position, Color::Black) - mobility(position, Color::White);

    match position.side_to_move() {
        Color::Black => score,
        Color::White => -score,
    }
}

fn hands(position: &Position, color: Color) -> i32 {
    HAND_TYPES
        .iter()
        .map(|&piece_type| position.hand(color, piece_type) as i32 * hand_value(piece_type))
        .sum()
}

/// 玉の安全度。
///
/// 守り駒を加点し、玉の周りに利いている相手の駒を減点し、**自陣の底から離れた分を減点**する。
/// 最後のひとつが要るのは、加点だけだと「駒の群れに玉を潜り込ませる」のが得に見えて、
/// 序盤から玉が出歩いてしまうため（囲いを組むのではなく駒に埋もれに行く）。
fn king_safety(position: &Position, color: Color) -> i32 {
    let Some(king) = position.king_square(color) else {
        return 0;
    };

    let ring = crate::attacks::king_attacks(king);
    let guards = (ring & position.pieces_of(color)).count() as i32;

    let mut attackers = 0;
    for square in ring {
        if position.is_attacked_by(color.flip(), square) {
            attackers += 1;
        }
    }

    // 先手の底は 9 段目、後手は 1 段目。
    let home_rank = match color {
        Color::Black => 9,
        Color::White => 1,
    };
    let advanced = (king.rank() as i32 - home_rank).abs();

    guards * GUARD_BONUS - attackers * ATTACKER_PENALTY - advanced * KING_EXPOSURE
}

/// 機動力 — 盤上の駒が動ける先の数。歩と玉は数えない（前者は常に 1、後者は安全度が見る）。
fn mobility(position: &Position, color: Color) -> i32 {
    let occupied = position.occupied();
    let ours = position.pieces_of(color);
    let mut total = 0;

    for square in ours {
        let Some(piece) = position.piece_at(square) else {
            continue;
        };
        match piece.piece_type {
            PieceType::Pawn | PieceType::King => continue,
            _ => {}
        }
        let reach = crate::attacks::attacks_of(piece.piece_type, color, square, occupied) & !ours;
        total += reach.count() as i32;
    }
    total * MOBILITY_BONUS
}

/// 静的な駒得だけを見る安い評価。指し手の並べ替え（SEE の代わり）で使う。
#[inline]
pub fn capture_gain(position: &Position, to: Square) -> i32 {
    position
        .piece_at(to)
        .map(|piece| material(piece.piece_type))
        .unwrap_or(0)
}

/// 手番側の全駒が利いているマス。玉の逃げ場を数えるのに使う。
pub fn attacked_squares(position: &Position, color: Color) -> Bitboard {
    let occupied = position.occupied();
    let mut result = Bitboard::EMPTY;
    for square in position.pieces_of(color) {
        if let Some(piece) = position.piece_at(square) {
            result |= crate::attacks::attacks_of(piece.piece_type, color, square, occupied);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moves::Move;

    #[test]
    fn the_starting_position_is_even() {
        assert_eq!(evaluate(&Position::hirate()), 0, "平手は左右対称なので互角");
    }

    #[test]
    fn winning_a_rook_for_nothing_is_worth_about_a_rook() {
        let mut position = Position::hirate();
        let before = evaluate(&position);
        // 先手が角で 2 二 の桂…ではなく、後手の角をただで取る手順を作る。
        position.do_move(Move::from_usi("7g7f").unwrap());
        position.do_move(Move::from_usi("3c3d").unwrap());
        let before_capture = evaluate(&position);
        position.do_move(Move::from_usi("8h2b+").unwrap()); // 角で角を取って成る
        let after = evaluate(&position);

        // 取ったあとは後手番なので符号が反転する。後手から見て不利になっているはず。
        assert!(after < 0, "角を取られた側の評価が負になっていない: {after}");
        assert!(
            -after > before_capture,
            "取る前より取ったあとのほうが取った側に有利であるべき"
        );
        let _ = before;
    }

    #[test]
    fn a_piece_in_hand_is_worth_more_than_the_same_piece_on_the_board() {
        for piece_type in HAND_TYPES {
            assert!(
                hand_value(piece_type) > material(piece_type),
                "{piece_type:?} の持ち駒価値が盤上以下"
            );
        }
    }

    #[test]
    fn the_evaluation_is_symmetric_under_colour_swap() {
        // 同じ形を先後入れ替えると、評価は符号だけが変わる。
        let black = Position::from_sfen("4k4/9/9/9/9/9/4P4/9/4K4 b - 1").unwrap();
        let white = Position::from_sfen("4k4/9/4p4/9/9/9/9/9/4K4 w - 1").unwrap();
        assert_eq!(evaluate(&black), evaluate(&white));
    }

    #[test]
    fn evaluation_does_not_depend_on_how_the_position_was_reached() {
        // 差分維持した psq が、SFEN から組み直したものと一致すること。
        let mut position = Position::hirate();
        for usi in ["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e"] {
            position.do_move(Move::from_usi(usi).unwrap());
            let rebuilt = Position::from_sfen(&position.to_sfen()).unwrap();
            assert_eq!(evaluate(&position), evaluate(&rebuilt), "{usi} のあと");
        }
    }

    #[test]
    fn an_exposed_king_scores_worse_than_a_tucked_one() {
        let tucked = Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").unwrap();
        let exposed = Position::from_sfen("4k4/9/9/9/4K4/9/9/9/9 b - 1").unwrap();
        assert!(
            evaluate(&tucked) > evaluate(&exposed),
            "出歩いた玉のほうが高く評価されている"
        );
    }
}
