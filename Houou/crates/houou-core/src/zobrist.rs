//! Zobrist ハッシュ — 局面を 64 ビットに畳む。
//!
//! 置換表の索きと千日手判定が同じ鍵を使う。**持ち駒と手番も鍵に入れる**のが将棋の要点で、
//! これを落とすと「盤面は同じだが持ち駒が違う」局面を同一視して探索が壊れる。
//!
//! 乱数は固定 seed の splitmix64 で作る。外部 crate を引かないためと、
//! 実行ごとに鍵が変わると置換表の再現性が無くなってデバッグできないため。

use crate::types::{Color, PieceType, Square, NUM_HAND_TYPES, NUM_PIECE_TYPES, NUM_SQUARES};
use std::sync::LazyLock;

/// 持ち駒の枚数の上限 +1（歩は最大 18 枚）。
pub const MAX_HAND_COUNT: usize = 19;

struct Keys {
    piece: [[[u64; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM],
    hand: [[[u64; MAX_HAND_COUNT]; NUM_HAND_TYPES]; Color::NUM],
    side: u64,
}

/// splitmix64。状態 1 語で分布が良く、定数だけで書ける。
struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }
}

static KEYS: LazyLock<Keys> = LazyLock::new(|| {
    let mut rng = SplitMix64(0x486f_756f_755f_5368); // "Houou_Sh"
    let mut keys = Keys {
        piece: [[[0; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM],
        hand: [[[0; MAX_HAND_COUNT]; NUM_HAND_TYPES]; Color::NUM],
        side: 0,
    };
    for color in 0..Color::NUM {
        for piece_type in 0..NUM_PIECE_TYPES {
            for square in 0..NUM_SQUARES {
                keys.piece[color][piece_type][square] = rng.next();
            }
        }
        for piece_type in 0..NUM_HAND_TYPES {
            // 0 枚は鍵 0。「持っていない」を XOR で表す必要が無く、初期化が楽になる。
            for count in 1..MAX_HAND_COUNT {
                keys.hand[color][piece_type][count] = rng.next();
            }
        }
    }
    keys.side = rng.next();
    keys
});

#[inline]
pub fn piece(color: Color, piece_type: PieceType, square: Square) -> u64 {
    KEYS.piece[color.index()][piece_type.index()][square.index()]
}

#[inline]
pub fn hand(color: Color, piece_type: PieceType, count: u8) -> u64 {
    KEYS.hand[color.index()][piece_type.index()][count as usize]
}

/// 手番が後手であることを表す鍵。先手番では XOR しない。
#[inline]
pub fn side() -> u64 {
    KEYS.side
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_distinct() {
        let mut seen = std::collections::HashSet::new();
        for color in Color::ALL {
            for piece_type in PieceType::ALL {
                for square in Square::all() {
                    assert!(
                        seen.insert(piece(color, piece_type, square)),
                        "駒の鍵が衝突した"
                    );
                }
            }
            for piece_type in crate::types::HAND_TYPES {
                for count in 1..MAX_HAND_COUNT as u8 {
                    assert!(seen.insert(hand(color, piece_type, count)), "持ち駒の鍵が衝突した");
                }
            }
        }
        assert!(seen.insert(side()));
    }

    #[test]
    fn an_empty_hand_contributes_nothing() {
        for color in Color::ALL {
            for piece_type in crate::types::HAND_TYPES {
                assert_eq!(hand(color, piece_type, 0), 0);
            }
        }
    }
}
