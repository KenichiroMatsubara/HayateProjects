//! perft — 深さ n までの合法手の総数を数える。
//!
//! **合法手生成が正しいかを判定する唯一の客観的なものさし**。評価も探索も、この数が
//! 合っていない限り意味を持たない。公開されている平手の値と 1 つでも食い違ったら、
//! それは高速化ではなくバグ。CI で回し続けること。

use crate::moves::{Move, MoveList};
use crate::movegen::generate_legal_into;
use crate::position::Position;

/// 平手初期局面の既知の perft 値（深さ 1 から）。
///
/// 出典は将棋の perft として広く引かれている値。ここを書き換えて通すのは**禁止** —
/// 食い違ったら直すのは生成器のほう。
pub const HIRATE_PERFT: [u64; 6] = [30, 900, 25_470, 719_731, 19_861_490, 547_581_517];

/// 深さ `depth` までの葉の数。
pub fn perft(position: &mut Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut moves = MoveList::new();
    generate_legal_into(position, &mut moves);

    // 葉の 1 つ手前は数えるだけで良い。進めて戻す往復を丸ごと省ける。
    if depth == 1 {
        return moves.len() as u64;
    }

    let mut nodes = 0;
    for index in 0..moves.len() {
        let mv = moves[index];
        position.do_move(mv);
        nodes += perft(position, depth - 1);
        position.undo_move();
    }
    nodes
}

/// 初手ごとの内訳。食い違った深さでこれを両側で取り、差が出た枝を辿ると原因に着く。
pub fn perft_divide(position: &mut Position, depth: u32) -> Vec<(Move, u64)> {
    assert!(depth >= 1, "divide は深さ 1 以上でしか意味がない");
    let mut moves = MoveList::new();
    generate_legal_into(position, &mut moves);

    let mut breakdown = Vec::with_capacity(moves.len());
    for index in 0..moves.len() {
        let mv = moves[index];
        position.do_move(mv);
        let nodes = perft(position, depth - 1);
        position.undo_move();
        breakdown.push((mv, nodes));
    }
    breakdown.sort_by_key(|(mv, _)| mv.to_usi());
    breakdown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hirate_matches_the_published_counts_to_depth_three() {
        let mut position = Position::hirate();
        for (index, &expected) in HIRATE_PERFT.iter().take(3).enumerate() {
            let depth = index as u32 + 1;
            assert_eq!(perft(&mut position, depth), expected, "深さ {depth}");
        }
    }

    #[test]
    fn perft_leaves_the_position_untouched() {
        let mut position = Position::hirate();
        let before = position.to_sfen();
        perft(&mut position, 3);
        assert_eq!(position.to_sfen(), before);
        assert_eq!(position.history_len(), 0);
    }

    #[test]
    fn divide_sums_to_the_whole() {
        let mut position = Position::hirate();
        let breakdown = perft_divide(&mut position, 3);
        assert_eq!(breakdown.len(), HIRATE_PERFT[0] as usize);
        let total: u64 = breakdown.iter().map(|(_, nodes)| nodes).sum();
        assert_eq!(total, HIRATE_PERFT[2]);
    }

    /// 深さ 4・5 は数十秒かかるので既定では走らせない。
    /// `cargo test --release -- --ignored` で回す。
    #[test]
    #[ignore = "重い。--release で明示的に回す"]
    fn hirate_matches_the_published_counts_to_depth_five() {
        let mut position = Position::hirate();
        for (index, &expected) in HIRATE_PERFT.iter().take(5).enumerate() {
            let depth = index as u32 + 1;
            assert_eq!(perft(&mut position, depth), expected, "深さ {depth}");
        }
    }
}
