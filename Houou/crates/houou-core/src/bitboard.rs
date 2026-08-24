//! 81 マスを `u128` の下位 81 ビットで持つビットボード。
//!
//! 将棋盤は 81 マスで 64 ビットに収まらない。多くのエンジンは 64+64 の 2 レーンに割るが、
//! Rust は `u128` を言語として持っており、wasm32 でも LLVM が i64 ペアへ素直に落とすので、
//! **1 つの整数として書ける**。レーン境界の場合分けが要らないぶん、生成器の見通しが良い。
//!
//! 上位 47 ビットは常に 0 に保つ。`!`（補集合）は必ず `MASK` を掛けて返す。

use crate::types::{Square, NUM_SQUARES};
use core::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

/// 有効な 81 ビットだけを立てたマスク。
const MASK: u128 = (1u128 << NUM_SQUARES) - 1;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub struct Bitboard(u128);

impl Bitboard {
    pub const EMPTY: Bitboard = Bitboard(0);
    pub const ALL: Bitboard = Bitboard(MASK);

    #[inline]
    pub const fn from_square(square: Square) -> Bitboard {
        Bitboard(1u128 << square.index())
    }

    #[inline]
    pub const fn from_raw(bits: u128) -> Bitboard {
        Bitboard(bits & MASK)
    }

    /// 1 つの筋（9 マス）。マス添字が筋優先なので、筋は**連続した 9 ビット**になる。
    /// 二歩の判定がこれ 1 本で済むのが、この添字並びを選んだ理由のひとつ。
    #[inline]
    pub const fn file(file: u8) -> Bitboard {
        assert!(file >= 1 && file <= 9);
        Bitboard(0x1ffu128 << ((file - 1) * 9))
    }

    #[inline]
    pub const fn raw(self) -> u128 {
        self.0
    }

    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub const fn is_not_empty(self) -> bool {
        self.0 != 0
    }

    #[inline]
    pub const fn contains(self, square: Square) -> bool {
        self.0 & (1u128 << square.index()) != 0
    }

    #[inline]
    pub fn set(&mut self, square: Square) {
        self.0 |= 1u128 << square.index();
    }

    #[inline]
    pub fn clear(&mut self, square: Square) {
        self.0 &= !(1u128 << square.index());
    }

    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// 添字が最小のマス。空なら `None`。
    #[inline]
    pub const fn lsb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_index(self.0.trailing_zeros() as u8))
        }
    }

    /// 添字が最大のマス。空なら `None`。飛び駒の「負方向の最も近い駒」を取るのに使う。
    #[inline]
    pub const fn msb(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_index(
                (127 - self.0.leading_zeros()) as u8,
            ))
        }
    }

    /// 最下位ビットを取り出して落とす。
    #[inline]
    pub fn pop_lsb(&mut self) -> Option<Square> {
        let square = self.lsb()?;
        self.0 &= self.0 - 1;
        Some(square)
    }
}

/// `for square in bitboard` で立っているマスを昇順に舐められる。
impl Iterator for Bitboard {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        self.pop_lsb()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.count() as usize;
        (n, Some(n))
    }
}

impl BitOr for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 | rhs.0)
    }
}

impl BitAnd for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitand(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 & rhs.0)
    }
}

impl BitXor for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn bitxor(self, rhs: Bitboard) -> Bitboard {
        Bitboard(self.0 ^ rhs.0)
    }
}

/// 補集合。**必ず 81 ビットへ切り詰める** — 上位ビットが漏れると、
/// 「盤外に駒がある」ことになって生成器が静かに壊れる。
impl Not for Bitboard {
    type Output = Bitboard;
    #[inline]
    fn not(self) -> Bitboard {
        Bitboard(!self.0 & MASK)
    }
}

impl BitOrAssign for Bitboard {
    #[inline]
    fn bitor_assign(&mut self, rhs: Bitboard) {
        self.0 |= rhs.0;
    }
}

impl BitAndAssign for Bitboard {
    #[inline]
    fn bitand_assign(&mut self, rhs: Bitboard) {
        self.0 &= rhs.0;
    }
}

impl BitXorAssign for Bitboard {
    #[inline]
    fn bitxor_assign(&mut self, rhs: Bitboard) {
        self.0 ^= rhs.0;
    }
}

impl core::fmt::Display for Bitboard {
    /// 盤の形に並べて出す（左が 9 筋、上が 1 段）。デバッグ専用。
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for rank in 1..=9u8 {
            for file in (1..=9u8).rev() {
                let square = Square::new(file, rank);
                f.write_str(if self.contains(square) { "* " } else { ". " })?;
            }
            writeln!(f)?;
        }
        Ok(())
    }
}
