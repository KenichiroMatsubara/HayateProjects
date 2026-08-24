//! 駒の利き。歩みの駒は事前計算した表、飛び駒は**レイと最も近い遮蔽駒**で出す。
//!
//! # なぜ magic bitboard ではないのか（今は）
//!
//! magic は表が数百 KB〜MB になり、wasm の初回ロードと初期化に効く。ここは
//! **正しさを先に固める段**なので、表が小さく読んで明らかなレイ方式を採る。
//! `houou bench` が movegen を律速だと言い出したら Qugiy 方式（128bit の加減算で
//! 飛び利きを出す）へ差し替える。差し替え先はこの module の中だけで閉じる。

use crate::bitboard::Bitboard;
use crate::types::{Color, PieceType, Square, NUM_SQUARES};
use std::sync::LazyLock;

/// 8 方向の (筋の増分, 段の増分)。
///
/// **奇数添字が「マス添字が増える向き」になるよう並べてある**（`index = (筋-1)*9 + (段-1)`
/// なので南 `+1`・西 `+9`・北西 `+8`・南西 `+10`）。遮蔽駒を lsb で取るか msb で取るかを
/// `dir & 1` だけで決められる。
const DIRECTIONS: [(i8, i8); 8] = [
    (0, -1),  // 0: 北（1 段目へ）
    (0, 1),   // 1: 南
    (-1, 0),  // 2: 東（1 筋へ）
    (1, 0),   // 3: 西
    (-1, -1), // 4: 北東
    (1, -1),  // 5: 北西
    (-1, 1),  // 6: 南東
    (1, 1),   // 7: 南西
];

const NORTH: usize = 0;
const SOUTH: usize = 1;
const ROOK_DIRS: [usize; 4] = [0, 1, 2, 3];
const BISHOP_DIRS: [usize; 4] = [4, 5, 6, 7];

struct Tables {
    pawn: [[Bitboard; NUM_SQUARES]; Color::NUM],
    knight: [[Bitboard; NUM_SQUARES]; Color::NUM],
    silver: [[Bitboard; NUM_SQUARES]; Color::NUM],
    gold: [[Bitboard; NUM_SQUARES]; Color::NUM],
    king: [Bitboard; NUM_SQUARES],
    /// `ray[dir][sq]` = sq からその向きに盤端まで伸びるマス群（sq 自身は含まない）。
    ray: [[Bitboard; NUM_SQUARES]; 8],
}

static TABLES: LazyLock<Tables> = LazyLock::new(Tables::build);

impl Tables {
    fn build() -> Tables {
        let mut tables = Tables {
            pawn: [[Bitboard::EMPTY; NUM_SQUARES]; Color::NUM],
            knight: [[Bitboard::EMPTY; NUM_SQUARES]; Color::NUM],
            silver: [[Bitboard::EMPTY; NUM_SQUARES]; Color::NUM],
            gold: [[Bitboard::EMPTY; NUM_SQUARES]; Color::NUM],
            king: [Bitboard::EMPTY; NUM_SQUARES],
            ray: [[Bitboard::EMPTY; NUM_SQUARES]; 8],
        };

        for square in Square::all() {
            let file = square.file() as i8;
            let rank = square.rank() as i8;
            let index = square.index();

            for color in Color::ALL {
                // 前進の向き。先手は段が減る。
                let f = color.forward();
                let c = color.index();

                tables.pawn[c][index] = steps(file, rank, &[(0, f)]);
                tables.knight[c][index] = steps(file, rank, &[(-1, 2 * f), (1, 2 * f)]);
                tables.silver[c][index] =
                    steps(file, rank, &[(-1, f), (0, f), (1, f), (-1, -f), (1, -f)]);
                tables.gold[c][index] = steps(
                    file,
                    rank,
                    &[(-1, f), (0, f), (1, f), (-1, 0), (1, 0), (0, -f)],
                );
            }

            tables.king[index] = steps(
                file,
                rank,
                &[
                    (-1, -1),
                    (0, -1),
                    (1, -1),
                    (-1, 0),
                    (1, 0),
                    (-1, 1),
                    (0, 1),
                    (1, 1),
                ],
            );

            for (dir, (df, dr)) in DIRECTIONS.iter().enumerate() {
                let mut ray = Bitboard::EMPTY;
                let (mut f, mut r) = (file + df, rank + dr);
                while let Some(step) = Square::try_new(f, r) {
                    ray.set(step);
                    f += df;
                    r += dr;
                }
                tables.ray[dir][index] = ray;
            }
        }

        tables
    }
}

/// 起点から 1 歩だけ進む相対座標の集まりを、盤内に落としてビットボードにする。
fn steps(file: i8, rank: i8, deltas: &[(i8, i8)]) -> Bitboard {
    let mut bitboard = Bitboard::EMPTY;
    for (df, dr) in deltas {
        if let Some(square) = Square::try_new(file + df, rank + dr) {
            bitboard.set(square);
        }
    }
    bitboard
}

/// 1 方向の飛び利き。最も近い遮蔽駒までを含めて返す（その駒は取れるので含める）。
#[inline]
fn ray_attacks(dir: usize, square: Square, occupied: Bitboard) -> Bitboard {
    let ray = TABLES.ray[dir][square.index()];
    let blockers = ray & occupied;
    // 添字が増える向きなら最も手前の遮蔽駒は lsb、減る向きなら msb。
    let nearest = if dir & 1 == 1 {
        blockers.lsb()
    } else {
        blockers.msb()
    };
    match nearest {
        None => ray,
        // 遮蔽駒より先だけを落とす。遮蔽駒自身は残る。
        Some(blocker) => ray ^ TABLES.ray[dir][blocker.index()],
    }
}

#[inline]
pub fn pawn_attacks(color: Color, square: Square) -> Bitboard {
    TABLES.pawn[color.index()][square.index()]
}

#[inline]
pub fn knight_attacks(color: Color, square: Square) -> Bitboard {
    TABLES.knight[color.index()][square.index()]
}

#[inline]
pub fn silver_attacks(color: Color, square: Square) -> Bitboard {
    TABLES.silver[color.index()][square.index()]
}

#[inline]
pub fn gold_attacks(color: Color, square: Square) -> Bitboard {
    TABLES.gold[color.index()][square.index()]
}

#[inline]
pub fn king_attacks(square: Square) -> Bitboard {
    TABLES.king[square.index()]
}

#[inline]
pub fn lance_attacks(color: Color, square: Square, occupied: Bitboard) -> Bitboard {
    let dir = match color {
        Color::Black => NORTH,
        Color::White => SOUTH,
    };
    ray_attacks(dir, square, occupied)
}

#[inline]
pub fn bishop_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    BISHOP_DIRS
        .iter()
        .fold(Bitboard::EMPTY, |acc, &dir| acc | ray_attacks(dir, square, occupied))
}

#[inline]
pub fn rook_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    ROOK_DIRS
        .iter()
        .fold(Bitboard::EMPTY, |acc, &dir| acc | ray_attacks(dir, square, occupied))
}

#[inline]
pub fn horse_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    bishop_attacks(square, occupied) | king_attacks(square)
}

#[inline]
pub fn dragon_attacks(square: Square, occupied: Bitboard) -> Bitboard {
    rook_attacks(square, occupied) | king_attacks(square)
}

/// 駒種と色から利きを引く唯一の入口。
///
/// 生成器も王手判定もここを通す。駒種ごとの場合分けが 2 箇所に散らないようにするため。
pub fn attacks_of(piece_type: PieceType, color: Color, square: Square, occupied: Bitboard) -> Bitboard {
    match piece_type {
        PieceType::Pawn => pawn_attacks(color, square),
        PieceType::Lance => lance_attacks(color, square, occupied),
        PieceType::Knight => knight_attacks(color, square),
        PieceType::Silver => silver_attacks(color, square),
        PieceType::Bishop => bishop_attacks(square, occupied),
        PieceType::Rook => rook_attacks(square, occupied),
        PieceType::Gold
        | PieceType::ProPawn
        | PieceType::ProLance
        | PieceType::ProKnight
        | PieceType::ProSilver => gold_attacks(color, square),
        PieceType::King => king_attacks(square),
        PieceType::Horse => horse_attacks(square, occupied),
        PieceType::Dragon => dragon_attacks(square, occupied),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_pawn_advances_one_rank_up() {
        let attacks = pawn_attacks(Color::Black, Square::new(7, 7));
        assert_eq!(attacks, Bitboard::from_square(Square::new(7, 6)));
    }

    #[test]
    fn white_pawn_advances_one_rank_down() {
        let attacks = pawn_attacks(Color::White, Square::new(3, 3));
        assert_eq!(attacks, Bitboard::from_square(Square::new(3, 4)));
    }

    #[test]
    fn pawn_on_the_far_edge_has_nowhere_to_go() {
        assert!(pawn_attacks(Color::Black, Square::new(5, 1)).is_empty());
        assert!(pawn_attacks(Color::White, Square::new(5, 9)).is_empty());
    }

    #[test]
    fn knight_jumps_two_forward_and_one_sideways() {
        let attacks = knight_attacks(Color::Black, Square::new(5, 5));
        assert_eq!(attacks.count(), 2);
        assert!(attacks.contains(Square::new(4, 3)));
        assert!(attacks.contains(Square::new(6, 3)));
    }

    #[test]
    fn gold_moves_six_ways_in_the_open() {
        assert_eq!(gold_attacks(Color::Black, Square::new(5, 5)).count(), 6);
    }

    #[test]
    fn silver_moves_five_ways_in_the_open() {
        assert_eq!(silver_attacks(Color::Black, Square::new(5, 5)).count(), 5);
    }

    #[test]
    fn king_moves_eight_ways_in_the_open_and_three_in_the_corner() {
        assert_eq!(king_attacks(Square::new(5, 5)).count(), 8);
        assert_eq!(king_attacks(Square::new(1, 1)).count(), 3);
    }

    #[test]
    fn rook_sweeps_the_whole_file_and_rank_when_unobstructed() {
        let attacks = rook_attacks(Square::new(5, 5), Bitboard::EMPTY);
        assert_eq!(attacks.count(), 16);
    }

    #[test]
    fn a_blocker_is_included_but_nothing_behind_it_is() {
        let blocker = Square::new(5, 3);
        let attacks = rook_attacks(Square::new(5, 5), Bitboard::from_square(blocker));
        assert!(attacks.contains(blocker), "遮蔽駒自身は取れるので含む");
        assert!(!attacks.contains(Square::new(5, 2)), "その先は届かない");
        assert!(attacks.contains(Square::new(5, 4)), "手前は届く");
    }

    #[test]
    fn bishop_sweeps_both_diagonals_and_stops_at_the_edge() {
        let attacks = bishop_attacks(Square::new(1, 1), Bitboard::EMPTY);
        assert_eq!(attacks.count(), 8, "1一 の角は 9九 まで 1 本だけ");
    }

    #[test]
    fn lance_runs_forward_only() {
        let attacks = lance_attacks(Color::Black, Square::new(1, 9), Bitboard::EMPTY);
        assert_eq!(attacks.count(), 8);
        assert!(attacks.contains(Square::new(1, 1)));
    }

    #[test]
    fn horse_and_dragon_add_the_king_step() {
        let horse = horse_attacks(Square::new(5, 5), Bitboard::EMPTY);
        assert_eq!(horse.count(), 16 + 4, "角の 16 マスに縦横 4 マスが足される");
        let dragon = dragon_attacks(Square::new(5, 5), Bitboard::EMPTY);
        assert_eq!(dragon.count(), 16 + 4, "飛の 16 マスに斜め 4 マスが足される");
    }
}
