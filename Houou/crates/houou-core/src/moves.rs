//! 指し手 — `u32` 1 語に畳む。
//!
//! ```text
//!   bit  0..=6   移動先
//!   bit  7..=13  移動元（打ちのときは未使用）
//!   bit 14       成る
//!   bit 15       打ち
//!   bit 16..=19  打つ駒種（打ちのときだけ）
//! ```
//!
//! 取った駒は**入れない**。`undo` に要る情報は `Position` 側の履歴スタックが持つ。
//! 指し手は探索で数百万個を並べ替える対象なので、1 語に収まることを優先する。

use crate::types::{PieceType, Square};

#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Move(u32);

const TO_SHIFT: u32 = 0;
const FROM_SHIFT: u32 = 7;
const SQUARE_MASK: u32 = 0x7f;
const PROMOTE_BIT: u32 = 1 << 14;
const DROP_BIT: u32 = 1 << 15;
const DROP_TYPE_SHIFT: u32 = 16;
const DROP_TYPE_MASK: u32 = 0xf;

impl Move {
    /// 盤上の駒を動かす手。
    #[inline]
    pub const fn board(from: Square, to: Square, promote: bool) -> Move {
        let mut bits = ((to.index() as u32) << TO_SHIFT) | ((from.index() as u32) << FROM_SHIFT);
        if promote {
            bits |= PROMOTE_BIT;
        }
        Move(bits)
    }

    /// 持ち駒を打つ手。
    #[inline]
    pub const fn drop(piece_type: PieceType, to: Square) -> Move {
        Move(
            ((to.index() as u32) << TO_SHIFT)
                | DROP_BIT
                | ((piece_type as u32) << DROP_TYPE_SHIFT),
        )
    }

    #[inline]
    pub const fn to(self) -> Square {
        Square::from_index(((self.0 >> TO_SHIFT) & SQUARE_MASK) as u8)
    }

    /// 移動元。打ちの手なら `None`。
    #[inline]
    pub const fn from(self) -> Option<Square> {
        if self.is_drop() {
            None
        } else {
            Some(Square::from_index(
                ((self.0 >> FROM_SHIFT) & SQUARE_MASK) as u8,
            ))
        }
    }

    #[inline]
    pub const fn is_drop(self) -> bool {
        self.0 & DROP_BIT != 0
    }

    #[inline]
    pub const fn is_promotion(self) -> bool {
        self.0 & PROMOTE_BIT != 0
    }

    /// 打つ駒種。打ちの手でなければ `None`。
    #[inline]
    pub const fn dropped_piece(self) -> Option<PieceType> {
        if self.is_drop() {
            Some(PieceType::from_index(
                ((self.0 >> DROP_TYPE_SHIFT) & DROP_TYPE_MASK) as u8,
            ))
        } else {
            None
        }
    }

    /// USI 表記（`7g7f` / `7g7f+` / `P*7f`）。
    pub fn to_usi(self) -> String {
        match self.dropped_piece() {
            Some(piece_type) => format!("{}*{}", piece_type.usi_char(), self.to().to_usi()),
            None => {
                let from = self.from().expect("打ちでなければ移動元がある");
                let mut text = format!("{}{}", from.to_usi(), self.to().to_usi());
                if self.is_promotion() {
                    text.push('+');
                }
                text
            }
        }
    }

    /// USI 表記を読む。**盤とは突き合わせない**ので、合法かどうかは呼ぶ側が確かめること。
    pub fn from_usi(text: &str) -> Option<Move> {
        let bytes = text.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b'*' {
            let piece_type = PieceType::from_usi_char(bytes[0] as char)?;
            let to = Square::from_usi(text.get(2..4)?)?;
            return Some(Move::drop(piece_type, to));
        }
        let from = Square::from_usi(text.get(0..2)?)?;
        let to = Square::from_usi(text.get(2..4)?)?;
        let promote = match text.len() {
            4 => false,
            5 if bytes[4] == b'+' => true,
            _ => return None,
        };
        Some(Move::board(from, to, promote))
    }
}

impl core::fmt::Debug for Move {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.to_usi())
    }
}

impl core::fmt::Display for Move {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.to_usi())
    }
}

/// 1 局面ぶんの指し手を積む固定長の入れ物。
///
/// 将棋で**合法手**が最も多い局面は 593 手。ここに積むのは自玉が取られる手も混じる
/// 疑似合法手なので、その上に余裕を取って 1024 にしてある。確保だけしておけば
/// 探索中にヒープを触らずに済む（探索は 1 秒に数万回この生成を回す）。
pub const MAX_MOVES: usize = 1024;

#[derive(Clone)]
pub struct MoveList {
    moves: [Move; MAX_MOVES],
    len: usize,
}

impl MoveList {
    #[inline]
    pub fn new() -> MoveList {
        MoveList {
            moves: [Move::default(); MAX_MOVES],
            len: 0,
        }
    }

    #[inline]
    pub fn push(&mut self, mv: Move) {
        debug_assert!(self.len < MAX_MOVES, "合法手が {MAX_MOVES} を超えた");
        self.moves[self.len] = mv;
        self.len += 1;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn as_slice(&self) -> &[Move] {
        &self.moves[..self.len]
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        &mut self.moves[..self.len]
    }

    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    #[inline]
    pub fn contains(&self, mv: Move) -> bool {
        self.as_slice().contains(&mv)
    }
}

impl Default for MoveList {
    fn default() -> MoveList {
        MoveList::new()
    }
}

impl core::ops::Deref for MoveList {
    type Target = [Move];
    #[inline]
    fn deref(&self) -> &[Move] {
        self.as_slice()
    }
}

impl core::ops::DerefMut for MoveList {
    #[inline]
    fn deref_mut(&mut self) -> &mut [Move] {
        self.as_mut_slice()
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = core::slice::Iter<'a, Move>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn board_move_round_trips_through_usi() {
        let mv = Move::board(Square::new(7, 7), Square::new(7, 6), false);
        assert_eq!(mv.to_usi(), "7g7f");
        assert_eq!(Move::from_usi("7g7f"), Some(mv));
    }

    #[test]
    fn promotion_round_trips_through_usi() {
        let mv = Move::board(Square::new(8, 8), Square::new(2, 2), true);
        assert_eq!(mv.to_usi(), "8h2b+");
        assert_eq!(Move::from_usi("8h2b+"), Some(mv));
        assert!(mv.is_promotion());
        assert!(!mv.is_drop());
    }

    #[test]
    fn drop_round_trips_through_usi() {
        let mv = Move::drop(PieceType::Pawn, Square::new(7, 6));
        assert_eq!(mv.to_usi(), "P*7f");
        assert_eq!(Move::from_usi("P*7f"), Some(mv));
        assert_eq!(mv.dropped_piece(), Some(PieceType::Pawn));
        assert_eq!(mv.from(), None);
    }

    #[test]
    fn every_hand_piece_survives_the_encoding() {
        for piece_type in crate::types::HAND_TYPES {
            let mv = Move::drop(piece_type, Square::new(5, 5));
            assert_eq!(mv.dropped_piece(), Some(piece_type));
        }
    }

    #[test]
    fn every_square_pair_survives_the_encoding() {
        for from in Square::all() {
            for to in Square::all() {
                let mv = Move::board(from, to, true);
                assert_eq!(mv.from(), Some(from));
                assert_eq!(mv.to(), to);
                assert!(mv.is_promotion());
                assert!(!mv.is_drop());
            }
        }
    }

    #[test]
    fn garbage_usi_is_rejected() {
        assert_eq!(Move::from_usi(""), None);
        assert_eq!(Move::from_usi("7g"), None);
        assert_eq!(Move::from_usi("7g7f++"), None);
        assert_eq!(Move::from_usi("0g7f"), None);
    }
}
