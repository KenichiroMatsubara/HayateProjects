//! 合法手生成。
//!
//! # 二段構え
//!
//! 1. **疑似合法手** — 駒の動きと、`to` だけを見れば判る禁じ手を弾く。
//!    二歩・行き所のない駒はここで落とす（局面を進めなくても判るので）。
//! 2. **合法性の漉し** — 実際に指してみて自玉が取られないか見る。打ち歩詰めもここ。
//!
//! 2 は「指して・戻す」なので素朴だが**必ず正しい**。ピンの事前計算による高速化は
//! perft が一致してから入れる。速い間違いより遅い正しさを先に置く。

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::moves::{Move, MoveList};
use crate::position::Position;
use crate::types::{PieceType, HAND_TYPES};

/// 合法手をすべて `list` に積む（`list` は空にしてから使う）。
pub fn generate_legal_into(position: &mut Position, list: &mut MoveList) {
    let mut pseudo = MoveList::new();
    generate_pseudo_legal(position, &mut pseudo);

    *list = MoveList::new();
    for index in 0..pseudo.len() {
        let mv = pseudo[index];
        if is_legal(position, mv) {
            list.push(mv);
        }
    }
}

/// 合法手をすべて返す。呼びやすさのための包み。
pub fn generate_legal(position: &mut Position) -> MoveList {
    let mut list = MoveList::new();
    generate_legal_into(position, &mut list);
    list
}

/// 手番側に合法手がひとつでもあるか。詰み判定はこれで足りる。
pub fn has_any_legal_move(position: &mut Position) -> bool {
    let mut pseudo = MoveList::new();
    generate_pseudo_legal(position, &mut pseudo);
    for index in 0..pseudo.len() {
        if is_legal(position, pseudo[index]) {
            return true;
        }
    }
    false
}

/// 詰んでいるか（王手がかかっていて合法手が無い）。
pub fn is_checkmate(position: &mut Position) -> bool {
    position.in_check() && !has_any_legal_move(position)
}

/// この手が今の局面で合法か。USI から来た指し手を盤に載せる前に通す。
pub fn is_legal_move(position: &mut Position, mv: Move) -> bool {
    let mut pseudo = MoveList::new();
    generate_pseudo_legal(position, &mut pseudo);
    pseudo.contains(mv) && is_legal(position, mv)
}

/// 疑似合法手をすべて `list` に積む。自玉が取られる手も混じる。
pub fn generate_pseudo_legal(position: &Position, list: &mut MoveList) {
    generate_board_moves(position, list);
    generate_drops(position, list);
}

/// 静止探索で読む手 — **取る手と成る手だけ**を合法手として積む。
///
/// 打つ手を含めないのは、駒を打つのは局面を「静か」にする手ではなく、静止探索が
/// 終わらなくなるため。王手がかかっているときはこれでは足りないので、
/// 呼ぶ側が [`generate_legal_into`] に切り替えること。
pub fn generate_noisy_into(position: &mut Position, list: &mut MoveList) {
    let mut pseudo = MoveList::new();
    generate_noisy_pseudo(position, &mut pseudo);

    list.clear();
    for index in 0..pseudo.len() {
        let mv = pseudo[index];
        // 取る手・成る手に打ち歩詰めは無いので、自玉の安全だけ見れば足りる。
        if !position.leaves_king_in_check(mv, position.side_to_move()) {
            list.push(mv);
        }
    }
}

fn generate_noisy_pseudo(position: &Position, list: &mut MoveList) {
    let us = position.side_to_move();
    let ours = position.pieces_of(us);
    let theirs = position.pieces_of(us.flip());
    let occupied = position.occupied();

    for from in ours {
        let piece = position
            .piece_at(from)
            .expect("自駒のビットが立つマスには駒が居る");
        let piece_type = piece.piece_type;
        let reach = attacks::attacks_of(piece_type, us, from, occupied) & !ours;
        let from_in_zone = us.is_promotion_rank(from.rank());

        // 取る手。成れるなら成りも不成も見る。
        for to in reach & theirs {
            if piece_type.can_promote() && (from_in_zone || us.is_promotion_rank(to.rank())) {
                list.push(Move::board(from, to, true));
            }
            if !piece_type.is_dead_at(us, to.rank()) {
                list.push(Move::board(from, to, false));
            }
        }

        // 取らない成り。成るだけで駒の価値が上がるので静止探索で見る値打ちがある。
        if piece_type.can_promote() {
            for to in reach & !theirs {
                if from_in_zone || us.is_promotion_rank(to.rank()) {
                    list.push(Move::board(from, to, true));
                }
            }
        }
    }
}

fn generate_board_moves(position: &Position, list: &mut MoveList) {
    let us = position.side_to_move();
    let ours = position.pieces_of(us);
    let occupied = position.occupied();

    for from in ours {
        let piece = position
            .piece_at(from)
            .expect("自駒のビットが立つマスには駒が居る");
        let piece_type = piece.piece_type;
        let targets = attacks::attacks_of(piece_type, us, from, occupied) & !ours;
        // 移動元が敵陣なら、どこへ動いても成れる。
        let from_in_zone = us.is_promotion_rank(from.rank());

        for to in targets {
            if piece_type.can_promote() && (from_in_zone || us.is_promotion_rank(to.rank())) {
                list.push(Move::board(from, to, true));
            }
            // 不成。行き所のない駒になるなら、そもそも指せない
            // （このとき成りは必ず生成済みなので、行き先が消えることはない）。
            if !piece_type.is_dead_at(us, to.rank()) {
                list.push(Move::board(from, to, false));
            }
        }
    }
}

fn generate_drops(position: &Position, list: &mut MoveList) {
    let us = position.side_to_move();
    let empty = !position.occupied();
    if empty.is_empty() {
        return;
    }

    // 二歩 — 自分の**歩**が居る筋には打てない。と金は数えない（`pieces` は成駒を含まない）。
    let mut pawn_files = Bitboard::EMPTY;
    for square in position.pieces(us, PieceType::Pawn) {
        pawn_files |= Bitboard::file(square.file());
    }

    for piece_type in HAND_TYPES {
        if position.hand(us, piece_type) == 0 {
            continue;
        }
        let targets = if piece_type == PieceType::Pawn {
            empty & !pawn_files
        } else {
            empty
        };
        for to in targets {
            // 行き所のない駒は打てない（歩・香は最終段、桂は最終 2 段）。
            if piece_type.is_dead_at(us, to.rank()) {
                continue;
            }
            list.push(Move::drop(piece_type, to));
        }
    }
}

/// 疑似合法手が本当に指せるか。
///
/// **自玉の安全は盤を進めずに見る**（`leaves_king_in_check`）。1 局面あたり数十〜数百回
/// 呼ばれるので、`do_move` / `undo_move` の往復（持ち駒・Zobrist 鍵・評価値の更新まで走る）を
/// 避けるのがそのまま生成の速度になる。
///
/// 実際に指してみるのは**打ち歩詰め**のときだけ。相手の合法手生成を呼ぶ必要があるので、
/// 「打った歩がそもそも王手になっている」ときに絞る。王手でない歩打ちは詰みになりようが
/// ないので、この足切りで再帰はほぼ起きない。
fn is_legal(position: &mut Position, mv: Move) -> bool {
    let us = position.side_to_move();
    if position.leaves_king_in_check(mv, us) {
        return false;
    }
    if mv.dropped_piece() != Some(PieceType::Pawn) {
        return true;
    }

    position.do_move(mv);
    let uchifuzume = position.in_check() && !has_any_legal_move(position);
    position.undo_move();
    !uchifuzume
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, Square};

    fn legal_moves(sfen: &str) -> Vec<String> {
        let mut position = Position::from_sfen(sfen).expect("読めるはず");
        let mut moves: Vec<String> = generate_legal(&mut position)
            .iter()
            .map(|mv| mv.to_usi())
            .collect();
        moves.sort();
        moves
    }

    #[test]
    fn hirate_has_thirty_legal_moves() {
        assert_eq!(legal_moves(crate::position::HIRATE_SFEN).len(), 30);
    }

    #[test]
    fn a_pawn_cannot_be_dropped_on_a_file_that_already_has_one() {
        // 先手は 5 筋にだけ歩を持ち、持ち駒に歩がある。
        let moves = legal_moves("4k4/9/9/9/9/9/4P4/9/4K4 b P 1");
        assert!(
            !moves.iter().any(|m| m.starts_with("P*5")),
            "5 筋には二歩で打てない: {moves:?}"
        );
        assert!(moves.iter().any(|m| m.starts_with("P*4")), "他の筋には打てる");
    }

    #[test]
    fn a_promoted_pawn_does_not_block_a_pawn_drop() {
        // 5 筋に居るのは と金 なので二歩にならない。
        let moves = legal_moves("4k4/9/9/9/9/9/4+P4/9/4K4 b P 1");
        assert!(
            moves.iter().any(|m| m.starts_with("P*5")),
            "と金は歩ではないので 5 筋に打てる: {moves:?}"
        );
    }

    #[test]
    fn pieces_cannot_be_dropped_where_they_could_never_move_again() {
        let moves = legal_moves("4k4/9/9/9/9/9/9/9/4K4 b PLN 1");
        assert!(!moves.contains(&"P*5a".to_string()), "歩は 1 段目に打てない");
        assert!(!moves.contains(&"L*5a".to_string()), "香は 1 段目に打てない");
        assert!(!moves.contains(&"N*5a".to_string()), "桂は 1 段目に打てない");
        assert!(!moves.contains(&"N*5b".to_string()), "桂は 2 段目にも打てない");
        assert!(moves.contains(&"P*5b".to_string()), "歩は 2 段目なら打てる");
        assert!(moves.contains(&"N*5c".to_string()), "桂は 3 段目なら打てる");
    }

    #[test]
    fn a_pawn_reaching_the_last_rank_must_promote() {
        // 後手玉は 1 一 に置いて、歩の行き先 5 一 を空けておく。
        let moves = legal_moves("8k/4P4/9/9/9/9/9/9/4K4 b - 1");
        assert!(moves.contains(&"5b5a+".to_string()));
        assert!(!moves.contains(&"5b5a".to_string()), "不成は指せない");
    }

    #[test]
    fn a_silver_may_decline_promotion() {
        let moves = legal_moves("4k4/9/4S4/9/9/9/9/9/4K4 b - 1");
        assert!(moves.contains(&"5c5b+".to_string()));
        assert!(moves.contains(&"5c5b".to_string()), "銀は不成も選べる");
    }

    #[test]
    fn leaving_your_own_king_in_check_is_not_a_move() {
        // 先手玉は 5 九、後手飛車が 5 一 から睨む。間の 5 五 の銀は 5 筋を外せない。
        let mut position = Position::from_sfen("4r4/9/9/9/4S4/9/9/9/4K4 b - 1").unwrap();
        let moves = generate_legal(&mut position);
        for mv in moves.iter() {
            let from = mv.from();
            if from == Some(Square::new(5, 5)) {
                assert_eq!(
                    mv.to().file(),
                    5,
                    "5 筋を外れると自玉が飛車に取られる: {}",
                    mv.to_usi()
                );
            }
        }
    }

    #[test]
    fn every_generated_move_leaves_our_king_safe() {
        let mut position = Position::from_sfen(
            "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1",
        )
        .unwrap();
        let us = position.side_to_move();
        let moves = generate_legal(&mut position);
        assert!(!moves.is_empty());
        for mv in moves.iter() {
            position.do_move(*mv);
            assert!(
                !position.is_in_check(us),
                "{} を指すと自玉が取られる",
                mv.to_usi()
            );
            position.undo_move();
        }
    }

    /// 打ち歩詰めの形。
    ///
    /// 後手玉 1 一。1 三 の金が 1 二 と 2 二 を、2 九 の飛車が 2 筋（2 一・2 二）を抑える。
    /// ここで 1 二 に歩を打つと、玉は 2 一 にも 2 二 にも行けず、金が利いているので
    /// 歩も取れない = 詰み。歩以外なら同じ形でも合法。
    const UCHIFUZUME_BOARD: &str = "8k/9/8G/9/9/9/9/9/K6R1";

    #[test]
    fn a_pawn_drop_that_would_be_checkmate_is_illegal() {
        let sfen = format!("{UCHIFUZUME_BOARD} b P 1");
        let mut position = Position::from_sfen(&sfen).unwrap();
        assert!(!position.is_in_check(Color::White), "打つ前に王手であってはならない");

        // 前提の確認 — この形が本当に詰みであること。崩れたらテストの土台が消える。
        position.do_move(Move::from_usi("P*1b").unwrap());
        let mated = is_checkmate(&mut position);
        position.undo_move();
        assert!(mated, "1 二 歩は詰みのはず");

        let moves = legal_moves(&sfen);
        assert!(
            !moves.contains(&"P*1b".to_string()),
            "打ち歩詰めなので指せない: {moves:?}"
        );
        assert!(
            moves.contains(&"P*1d".to_string()),
            "詰みにならない歩打ちは指せる"
        );
    }

    #[test]
    fn dropping_a_piece_other_than_a_pawn_for_checkmate_is_fine() {
        // 同じ形に香を打つのは合法。打ち歩詰めは歩だけの禁じ手。
        let sfen = format!("{UCHIFUZUME_BOARD} b L 1");
        let mut position = Position::from_sfen(&sfen).unwrap();
        position.do_move(Move::from_usi("L*1b").unwrap());
        let mated = is_checkmate(&mut position);
        position.undo_move();
        assert!(mated, "香でも同じ形は詰み");

        assert!(
            legal_moves(&sfen).contains(&"L*1b".to_string()),
            "香なら詰ませても構わない"
        );
    }

    #[test]
    fn a_king_in_check_must_answer_the_check() {
        // 9 九 から数えて飛車は 4 九。4 一 へ成り込むと、隣の 5 一 の玉に竜の王手。
        let mut position = Position::from_sfen("4k4/9/9/9/9/9/9/9/4KR3 b - 1").unwrap();
        position.do_move(Move::from_usi("4i4a+").unwrap());
        // すべての合法手が王手を外していること。
        assert!(position.in_check());
        let us = position.side_to_move();
        let moves = generate_legal(&mut position);
        assert!(!moves.is_empty());
        for mv in moves.iter() {
            position.do_move(*mv);
            assert!(!position.is_in_check(us), "{} は王手を外していない", mv.to_usi());
            position.undo_move();
        }
    }

    #[test]
    fn a_lone_king_facing_nothing_has_moves_for_both_colors() {
        for (sfen, color) in [
            ("4k4/9/9/9/9/9/9/9/4K4 b - 1", Color::Black),
            ("4k4/9/9/9/9/9/9/9/4K4 w - 1", Color::White),
        ] {
            let mut position = Position::from_sfen(sfen).unwrap();
            assert_eq!(position.side_to_move(), color);
            assert_eq!(generate_legal(&mut position).len(), 5, "端の玉は 5 マスへ動ける");
        }
    }
}
