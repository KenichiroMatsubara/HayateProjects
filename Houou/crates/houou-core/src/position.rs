//! 局面 — 盤・持ち駒・手番と、`do_move` / `undo_move`。
//!
//! 盤は「駒種ごとのビットボード」と「マス→駒の配列」を**両方**持つ。前者は利きの計算に、
//! 後者は「このマスに何が居るか」を 1 命令で答えるのに要る。両者が食い違うと生成器が
//! 静かに壊れるので、更新は `put` / `remove` の 2 つだけを通す。Zobrist 鍵と
//! 駒割・駒位置の評価値もこの 2 つが同時に維持する。

use crate::attacks;
use crate::bitboard::Bitboard;
use crate::eval;
use crate::moves::Move;
use crate::types::{
    Color, Piece, PieceType, Square, HAND_TYPES, NUM_HAND_TYPES, NUM_PIECE_TYPES, NUM_SQUARES,
};
use crate::zobrist;

/// 平手の初期局面。
pub const HIRATE_SFEN: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

/// SFEN が読めなかった理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SfenError(pub String);

impl core::fmt::Display for SfenError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "SFEN として読めない: {}", self.0)
    }
}

impl std::error::Error for SfenError {}

/// 千日手の判定結果。**手番側から見た**結論を返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repetition {
    /// 千日手が成立していない。
    None,
    /// 同一局面 4 回。引き分け。
    Draw,
    /// 相手の連続王手による千日手。手番側の勝ち。
    Win,
    /// 自分の連続王手による千日手。手番側の負け。
    Loss,
}

/// 1 局面ぶんの、指し手からは復元できない情報。
#[derive(Clone, Copy, Debug)]
struct StateInfo {
    /// この局面へ来た手。根では未使用。
    mv: Move,
    /// 取った駒の**盤上での**駒種（成っていれば成ったまま）。
    captured: Option<PieceType>,
    /// この局面の Zobrist 鍵。千日手の照合に使う。
    key: u64,
    /// **この局面の手番側**に王手がかかっているか。`do_move` が 1 回だけ計算して置く。
    checked: bool,
    /// 直近の非可逆手（取る・打つ・成る）からの手数。千日手の走査範囲を切るために持つ。
    since_irreversible: u16,
}

#[derive(Clone)]
pub struct Position {
    board: [Option<Piece>; NUM_SQUARES],
    by_color: [Bitboard; Color::NUM],
    by_type: [Bitboard; NUM_PIECE_TYPES],
    occupied: Bitboard,
    hands: [[u8; NUM_HAND_TYPES]; Color::NUM],
    side_to_move: Color,
    move_number: u32,
    king: [Option<Square>; Color::NUM],
    /// Zobrist 鍵。`put` / `remove` / 持ち駒の増減 / 手番の反転がすべて XOR で維持する。
    key: u64,
    /// 駒割 + 駒位置の合計（**先手視点**）。`put` / `remove` が差分で維持する。
    psq: i32,
    /// 根を含む局面の履歴。常に 1 つ以上入っている。
    states: Vec<StateInfo>,
}

impl Position {
    /// 駒の無い盤。SFEN の読み取りが自分で埋める。
    fn empty() -> Position {
        Position {
            board: [None; NUM_SQUARES],
            by_color: [Bitboard::EMPTY; Color::NUM],
            by_type: [Bitboard::EMPTY; NUM_PIECE_TYPES],
            occupied: Bitboard::EMPTY,
            hands: [[0; NUM_HAND_TYPES]; Color::NUM],
            side_to_move: Color::Black,
            move_number: 1,
            king: [None; Color::NUM],
            key: 0,
            psq: 0,
            states: Vec::with_capacity(512),
        }
    }

    /// 平手の初期局面。
    pub fn hirate() -> Position {
        Position::from_sfen(HIRATE_SFEN).expect("平手 SFEN は定数なので必ず読める")
    }

    // ---- 参照 ----------------------------------------------------------

    #[inline]
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.board[square.index()]
    }

    #[inline]
    pub fn occupied(&self) -> Bitboard {
        self.occupied
    }

    #[inline]
    pub fn pieces_of(&self, color: Color) -> Bitboard {
        self.by_color[color.index()]
    }

    #[inline]
    pub fn pieces(&self, color: Color, piece_type: PieceType) -> Bitboard {
        self.by_type[piece_type.index()] & self.by_color[color.index()]
    }

    #[inline]
    pub fn pieces_all(&self, piece_type: PieceType) -> Bitboard {
        self.by_type[piece_type.index()]
    }

    /// 金と同じ動きをする駒すべて（金・と・成香・成桂・成銀）。
    #[inline]
    pub fn gold_like(&self, color: Color) -> Bitboard {
        (self.by_type[PieceType::Gold.index()]
            | self.by_type[PieceType::ProPawn.index()]
            | self.by_type[PieceType::ProLance.index()]
            | self.by_type[PieceType::ProKnight.index()]
            | self.by_type[PieceType::ProSilver.index()])
            & self.by_color[color.index()]
    }

    /// 持ち駒の枚数。
    ///
    /// # Panics
    /// 持ち駒になり得ない駒種（玉・成駒）を渡すと panic する。持ち駒配列は
    /// `Pawn..=Gold` の 7 要素しか無く、成駒の添字はそこに無い。
    #[inline]
    pub fn hand(&self, color: Color, piece_type: PieceType) -> u8 {
        debug_assert!(
            piece_type.index() < NUM_HAND_TYPES,
            "{piece_type:?} は持ち駒にならない"
        );
        self.hands[color.index()][piece_type.index()]
    }

    #[inline]
    pub fn side_to_move(&self) -> Color {
        self.side_to_move
    }

    #[inline]
    pub fn move_number(&self) -> u32 {
        self.move_number
    }

    #[inline]
    pub fn king_square(&self, color: Color) -> Option<Square> {
        self.king[color.index()]
    }

    /// この局面の Zobrist 鍵。
    #[inline]
    pub fn key(&self) -> u64 {
        self.key
    }

    /// 駒割 + 駒位置の合計（先手視点）。評価関数が使う。
    #[inline]
    pub fn psq_score(&self) -> i32 {
        self.psq
    }

    /// 根からの手数。
    #[inline]
    pub fn history_len(&self) -> usize {
        self.states.len() - 1
    }

    /// 直前の手。根なら `None`。
    #[inline]
    pub fn last_move(&self) -> Option<Move> {
        if self.states.len() <= 1 {
            None
        } else {
            Some(self.states[self.states.len() - 1].mv)
        }
    }

    // ---- 利きと王手 ----------------------------------------------------

    /// `square` に利いている `color` の駒すべて。
    ///
    /// 逆向きに引くのが要点 — 「s に居る色 c の駒種 T が sq に利く」は
    /// 「sq に置いた色 !c の T が s に利く」と同値なので、駒を全部舐めずに表 1 引きで済む。
    pub fn attackers_to(&self, color: Color, square: Square, occupied: Bitboard) -> Bitboard {
        let them = color.flip();
        let mut result = attacks::pawn_attacks(them, square) & self.pieces(color, PieceType::Pawn);
        result |= attacks::knight_attacks(them, square) & self.pieces(color, PieceType::Knight);
        result |= attacks::silver_attacks(them, square) & self.pieces(color, PieceType::Silver);
        result |= attacks::gold_attacks(them, square) & self.gold_like(color);
        result |= attacks::king_attacks(square) & self.pieces(color, PieceType::King);
        result |=
            attacks::lance_attacks(them, square, occupied) & self.pieces(color, PieceType::Lance);

        let diagonal = self.pieces(color, PieceType::Bishop) | self.pieces(color, PieceType::Horse);
        result |= attacks::bishop_attacks(square, occupied) & diagonal;

        let orthogonal = self.pieces(color, PieceType::Rook) | self.pieces(color, PieceType::Dragon);
        result |= attacks::rook_attacks(square, occupied) & orthogonal;

        // 馬と竜の「玉の 1 歩」ぶん。角・飛の利きには含まれない向きを拾う。
        let promoted_sliders =
            self.pieces(color, PieceType::Horse) | self.pieces(color, PieceType::Dragon);
        result |= attacks::king_attacks(square) & promoted_sliders;

        result
    }

    /// `square` が `color` の駒に狙われているか。
    #[inline]
    pub fn is_attacked_by(&self, color: Color, square: Square) -> bool {
        self.attackers_to(color, square, self.occupied).is_not_empty()
    }

    /// `color` の玉に王手がかかっているか（その場で計算する）。玉が盤上に無ければ `false`。
    #[inline]
    pub fn is_in_check(&self, color: Color) -> bool {
        match self.king[color.index()] {
            Some(king) => self.is_attacked_by(color.flip(), king),
            None => false,
        }
    }

    /// 手番側に王手がかかっているか。**`do_move` が計算済みの値を読むだけ**なので安い。
    #[inline]
    pub fn in_check(&self) -> bool {
        self.states[self.states.len() - 1].checked
    }

    /// 手番側の玉に利いている相手の駒。
    #[inline]
    pub fn checkers(&self) -> Bitboard {
        match self.king[self.side_to_move.index()] {
            Some(king) => self.attackers_to(self.side_to_move.flip(), king, self.occupied),
            None => Bitboard::EMPTY,
        }
    }

    /// `mv` を指すと `us` の玉が取られるか。**盤を進めずに**ビットボードだけで答える。
    ///
    /// 合法手生成の内側で 1 局面あたり数十〜数百回呼ばれるので、`do_move` / `undo_move` の
    /// 往復（持ち駒・鍵・評価値の更新まで走る）を避けるのが効く。
    pub fn leaves_king_in_check(&self, mv: Move, us: Color) -> bool {
        let to = mv.to();
        let to_bit = Bitboard::from_square(to);

        let (king, occupied_after) = match mv.from() {
            // 打ちは盤に駒が増えるだけ。玉は動かず、駒も取らない。
            None => (self.king[us.index()], self.occupied | to_bit),
            Some(from) => {
                let moving = self.board[from.index()].expect("移動元に駒が無い");
                let occupied_after =
                    (self.occupied ^ Bitboard::from_square(from)) | to_bit;
                let king = if moving.piece_type == PieceType::King {
                    Some(to)
                } else {
                    self.king[us.index()]
                };
                (king, occupied_after)
            }
        };

        let Some(king) = king else {
            return false;
        };

        // 取られた駒はもう利かない。`attackers_to` は盤を進めていないので
        // 取られる駒がまだ相手のビットボードに残っている — その 1 マスを落とす。
        let captured = if mv.from().is_some() { to_bit } else { Bitboard::EMPTY };
        (self.attackers_to(us.flip(), king, occupied_after) & !captured).is_not_empty()
    }

    // ---- 進める・戻す --------------------------------------------------

    /// 1 手進める。**合法性は確かめない**（生成器が保証する）。
    pub fn do_move(&mut self, mv: Move) {
        let us = self.side_to_move;
        let to = mv.to();
        let previous = self.states[self.states.len() - 1].since_irreversible;

        let (captured, irreversible) = match mv.dropped_piece() {
            Some(piece_type) => {
                debug_assert!(self.hand(us, piece_type) > 0, "持っていない駒を打とうとした");
                debug_assert!(self.piece_at(to).is_none(), "駒のあるマスへ打とうとした");
                self.sub_hand(us, piece_type);
                self.put(to, Piece::new(us, piece_type));
                (None, true)
            }
            None => {
                let from = mv.from().expect("打ちでなければ移動元がある");
                let moving = self.remove(from);
                let captured = self.piece_at(to).map(|piece| piece.piece_type);
                if let Some(captured_type) = captured {
                    self.remove(to);
                    // 成駒は成る前に戻して持ち駒へ。玉だけは持ち駒にならない
                    // （合法手では起きないが、不正な SFEN を渡されても落ちないようにする）。
                    if captured_type != PieceType::King {
                        self.add_hand(us, captured_type.unpromoted());
                    }
                }
                let placed = if mv.is_promotion() {
                    moving
                        .piece_type
                        .promoted()
                        .expect("成れない駒に成りの手が来た")
                } else {
                    moving.piece_type
                };
                self.put(to, Piece::new(us, placed));
                (captured, captured.is_some() || mv.is_promotion())
            }
        };

        self.side_to_move = us.flip();
        self.key ^= zobrist::side();
        self.move_number += 1;

        // 新しい手番側が王手を受けているかは、探索が毎ノード欲しがる。ここで 1 回だけ計算して持つ。
        let checked = self.is_in_check(self.side_to_move);
        self.states.push(StateInfo {
            mv,
            captured,
            key: self.key,
            checked,
            since_irreversible: if irreversible { 0 } else { previous.saturating_add(1) },
        });
    }

    /// 1 手戻す。`do_move` していなければ panic する。
    pub fn undo_move(&mut self) {
        assert!(self.states.len() > 1, "戻せる手が無い");
        let state = self.states.pop().expect("上で確かめた");
        // 直前に指したのは今の手番の相手。
        let us = self.side_to_move.flip();
        self.side_to_move = us;
        self.key ^= zobrist::side();
        self.move_number -= 1;

        let to = state.mv.to();
        match state.mv.dropped_piece() {
            Some(piece_type) => {
                self.remove(to);
                self.add_hand(us, piece_type);
            }
            None => {
                let from = state.mv.from().expect("打ちでなければ移動元がある");
                let moved = self.remove(to);
                let original = if state.mv.is_promotion() {
                    moved.piece_type.unpromoted()
                } else {
                    moved.piece_type
                };
                self.put(from, Piece::new(us, original));
                if let Some(captured_type) = state.captured {
                    self.put(to, Piece::new(us.flip(), captured_type));
                    if captured_type != PieceType::King {
                        self.sub_hand(us, captured_type.unpromoted());
                    }
                }
            }
        }
    }

    /// パス（null move）。**王手中に呼んではならない。**
    pub fn do_null_move(&mut self) {
        debug_assert!(!self.in_check(), "王手中に手番を渡すことはできない");
        self.side_to_move = self.side_to_move.flip();
        self.key ^= zobrist::side();
        self.move_number += 1;
        self.states.push(StateInfo {
            mv: Move::default(),
            captured: None,
            key: self.key,
            // 王手をかけていない側に手番を渡したので、王手はかかっていない。
            checked: false,
            // パスは局面を変えないが、千日手の照合で誤って一致させないよう区切る。
            since_irreversible: 0,
        });
    }

    pub fn undo_null_move(&mut self) {
        self.states.pop().expect("戻せるパスが無い");
        self.side_to_move = self.side_to_move.flip();
        self.key ^= zobrist::side();
        self.move_number -= 1;
    }

    // ---- 千日手 --------------------------------------------------------

    /// 千日手の判定。**手番側から見た**結論を返す。
    ///
    /// 同一局面（盤・持ち駒・手番がすべて同じ）が 4 回現れたら千日手。そのあいだ
    /// 一方が王手をかけ続けていたなら、かけていた側の負けになる。
    pub fn repetition(&self) -> Repetition {
        let last = self.states.len() - 1;
        let current_key = self.states[last].key;
        // 取る・打つ・成るのどれかが挟まれば、それ以前の局面とは二度と一致しない。
        let window = self.states[last].since_irreversible as usize;

        let mut repeats = 0;
        // 手番側が毎回王手を受けていたか / 相手が毎回王手を受けていたか。
        let mut we_are_checked_throughout = self.states[last].checked;
        let mut they_are_checked_throughout = true;

        for back in 1..=window.min(last) {
            let state = &self.states[last - back];
            if back % 2 == 0 {
                we_are_checked_throughout &= state.checked;
            } else {
                they_are_checked_throughout &= state.checked;
            }

            // 手番が一致するのは偶数手前だけ。
            if back % 2 == 0 && state.key == current_key {
                repeats += 1;
                if repeats >= 3 {
                    return if we_are_checked_throughout && !they_are_checked_throughout {
                        Repetition::Win
                    } else if they_are_checked_throughout && !we_are_checked_throughout {
                        Repetition::Loss
                    } else {
                        Repetition::Draw
                    };
                }
            }
        }
        Repetition::None
    }

    /// 探索の枝刈り用 — 現局面が過去に 1 度でも現れているか。
    /// 千日手の成立を待たずに切ると探索が速くなる。
    pub fn has_repeated(&self) -> bool {
        let last = self.states.len() - 1;
        let current_key = self.states[last].key;
        let limit = (self.states[last].since_irreversible as usize).min(last);
        // 手番が一致するのは偶数手前だけ。
        (2..=limit)
            .step_by(2)
            .any(|back| self.states[last - back].key == current_key)
    }

    // ---- 盤の更新はこの 2 つだけを通す ---------------------------------

    fn put(&mut self, square: Square, piece: Piece) {
        debug_assert!(self.board[square.index()].is_none(), "駒のあるマスへ置こうとした");
        self.board[square.index()] = Some(piece);
        let bit = Bitboard::from_square(square);
        self.by_color[piece.color.index()] |= bit;
        self.by_type[piece.piece_type.index()] |= bit;
        self.occupied |= bit;
        self.key ^= zobrist::piece(piece.color, piece.piece_type, square);
        self.psq += eval::psq_value(piece, square);
        if piece.piece_type == PieceType::King {
            self.king[piece.color.index()] = Some(square);
        }
    }

    fn remove(&mut self, square: Square) -> Piece {
        let piece = self.board[square.index()]
            .take()
            .expect("空マスから駒を取ろうとした");
        let mask = !Bitboard::from_square(square);
        self.by_color[piece.color.index()] &= mask;
        self.by_type[piece.piece_type.index()] &= mask;
        self.occupied &= mask;
        self.key ^= zobrist::piece(piece.color, piece.piece_type, square);
        self.psq -= eval::psq_value(piece, square);
        if piece.piece_type == PieceType::King {
            self.king[piece.color.index()] = None;
        }
        piece
    }

    fn add_hand(&mut self, color: Color, piece_type: PieceType) {
        let count = self.hands[color.index()][piece_type.index()];
        self.key ^= zobrist::hand(color, piece_type, count);
        self.hands[color.index()][piece_type.index()] = count + 1;
        self.key ^= zobrist::hand(color, piece_type, count + 1);
    }

    fn sub_hand(&mut self, color: Color, piece_type: PieceType) {
        let count = self.hands[color.index()][piece_type.index()];
        debug_assert!(count > 0, "持っていない駒を減らそうとした");
        self.key ^= zobrist::hand(color, piece_type, count);
        self.hands[color.index()][piece_type.index()] = count - 1;
        self.key ^= zobrist::hand(color, piece_type, count - 1);
    }

    // ---- SFEN ----------------------------------------------------------

    /// SFEN を読む。手数は省略できる（省略時は 1）。
    pub fn from_sfen(sfen: &str) -> Result<Position, SfenError> {
        let mut fields = sfen.split_whitespace();
        let board = fields
            .next()
            .ok_or_else(|| SfenError("盤の欄が無い".into()))?;
        let side = fields
            .next()
            .ok_or_else(|| SfenError("手番の欄が無い".into()))?;
        let hands = fields
            .next()
            .ok_or_else(|| SfenError("持ち駒の欄が無い".into()))?;
        let move_number = fields.next().unwrap_or("1");

        let mut position = Position::empty();
        position.parse_board(board)?;

        position.side_to_move = match side {
            "b" => Color::Black,
            "w" => Color::White,
            other => return Err(SfenError(format!("手番が `b` でも `w` でもない: {other}"))),
        };
        if position.side_to_move == Color::White {
            position.key ^= zobrist::side();
        }

        position.parse_hands(hands)?;

        position.move_number = move_number
            .parse()
            .map_err(|_| SfenError(format!("手数が数でない: {move_number}")))?;

        // 根の状態を積む。ここまでで鍵と評価値は `put` / `add_hand` が積み上げ済み。
        let checked = position.is_in_check(position.side_to_move);
        position.states.push(StateInfo {
            mv: Move::default(),
            captured: None,
            key: position.key,
            checked,
            since_irreversible: 0,
        });

        Ok(position)
    }

    fn parse_board(&mut self, field: &str) -> Result<(), SfenError> {
        let ranks: Vec<&str> = field.split('/').collect();
        if ranks.len() != 9 {
            return Err(SfenError(format!("段が 9 つ無い: {} 個", ranks.len())));
        }

        for (rank_index, rank_text) in ranks.iter().enumerate() {
            let rank = rank_index as u8 + 1;
            // SFEN は 9 筋から 1 筋へ並べる。
            let mut file: i8 = 9;
            let mut promoted = false;

            for c in rank_text.chars() {
                if c == '+' {
                    if promoted {
                        return Err(SfenError("`+` が続いている".into()));
                    }
                    promoted = true;
                    continue;
                }
                if let Some(skip) = c.to_digit(10) {
                    if promoted {
                        return Err(SfenError("`+` のあとが数字".into()));
                    }
                    file -= skip as i8;
                    continue;
                }

                let base = PieceType::from_usi_char(c.to_ascii_uppercase())
                    .ok_or_else(|| SfenError(format!("駒として読めない文字: {c}")))?;
                let piece_type = if promoted {
                    base.promoted()
                        .ok_or_else(|| SfenError(format!("成れない駒に `+` が付いている: {c}")))?
                } else {
                    base
                };
                let color = if c.is_ascii_uppercase() {
                    Color::Black
                } else {
                    Color::White
                };
                let square = Square::try_new(file, rank as i8)
                    .ok_or_else(|| SfenError(format!("{rank} 段目の駒が盤からはみ出した")))?;
                if self.board[square.index()].is_some() {
                    return Err(SfenError(format!("{rank} 段目で駒が重なった")));
                }
                self.put(square, Piece::new(color, piece_type));
                file -= 1;
                promoted = false;
            }

            if file != 0 {
                return Err(SfenError(format!("{rank} 段目の筋が 9 に足りない")));
            }
        }
        Ok(())
    }

    fn parse_hands(&mut self, field: &str) -> Result<(), SfenError> {
        if field == "-" {
            return Ok(());
        }
        let mut count: u32 = 0;
        for c in field.chars() {
            if let Some(digit) = c.to_digit(10) {
                count = count * 10 + digit;
                continue;
            }
            let piece_type = PieceType::from_usi_char(c.to_ascii_uppercase())
                .ok_or_else(|| SfenError(format!("持ち駒として読めない文字: {c}")))?;
            if piece_type == PieceType::King {
                return Err(SfenError("玉は持ち駒にならない".into()));
            }
            let color = if c.is_ascii_uppercase() {
                Color::Black
            } else {
                Color::White
            };
            let total = count.max(1);
            if total as usize >= zobrist::MAX_HAND_COUNT {
                return Err(SfenError(format!("持ち駒が多すぎる: {c} が {total} 枚")));
            }
            for _ in 0..total {
                self.add_hand(color, piece_type);
            }
            count = 0;
        }
        if count != 0 {
            return Err(SfenError("持ち駒の数字が駒に付いていない".into()));
        }
        Ok(())
    }

    /// SFEN を書く。`from_sfen` と往復する。
    pub fn to_sfen(&self) -> String {
        let mut text = String::with_capacity(64);

        for rank in 1..=9u8 {
            if rank > 1 {
                text.push('/');
            }
            let mut empty = 0;
            for file in (1..=9u8).rev() {
                match self.piece_at(Square::new(file, rank)) {
                    None => empty += 1,
                    Some(piece) => {
                        if empty > 0 {
                            text.push_str(&empty.to_string());
                            empty = 0;
                        }
                        text.push_str(&piece.to_sfen());
                    }
                }
            }
            if empty > 0 {
                text.push_str(&empty.to_string());
            }
        }

        text.push(' ');
        text.push(match self.side_to_move {
            Color::Black => 'b',
            Color::White => 'w',
        });

        text.push(' ');
        // 慣習の並び: 飛・角・金・銀・桂・香・歩、先手の持ち駒を先に。
        const HAND_ORDER: [PieceType; 7] = [
            PieceType::Rook,
            PieceType::Bishop,
            PieceType::Gold,
            PieceType::Silver,
            PieceType::Knight,
            PieceType::Lance,
            PieceType::Pawn,
        ];
        let mut any = false;
        for color in Color::ALL {
            for piece_type in HAND_ORDER {
                let count = self.hand(color, piece_type);
                if count == 0 {
                    continue;
                }
                any = true;
                if count > 1 {
                    text.push_str(&count.to_string());
                }
                let c = piece_type.usi_char();
                text.push(match color {
                    Color::Black => c,
                    Color::White => c.to_ascii_lowercase(),
                });
            }
        }
        if !any {
            text.push('-');
        }

        text.push(' ');
        text.push_str(&self.move_number.to_string());
        text
    }

    /// 開始局面と USI の指し手列から局面を組む（USI の `position` コマンドの形）。
    pub fn from_sfen_and_moves(sfen: &str, moves: &[&str]) -> Result<Position, SfenError> {
        let mut position = Position::from_sfen(sfen)?;
        for text in moves {
            let mv = Move::from_usi(text)
                .ok_or_else(|| SfenError(format!("指し手として読めない: {text}")))?;
            if !crate::movegen::is_legal_move(&mut position, mv) {
                return Err(SfenError(format!("合法手ではない: {text}")));
            }
            position.do_move(mv);
        }
        Ok(position)
    }

    /// 盤を人が読める形に。デバッグ専用。
    pub fn to_ascii(&self) -> String {
        let mut text = String::new();
        text.push_str("  9  8  7  6  5  4  3  2  1\n");
        for rank in 1..=9u8 {
            for file in (1..=9u8).rev() {
                match self.piece_at(Square::new(file, rank)) {
                    None => text.push_str(" . "),
                    Some(piece) => {
                        let s = piece.to_sfen();
                        if s.len() == 1 {
                            text.push(' ');
                        }
                        text.push_str(&s);
                        text.push(' ');
                    }
                }
            }
            text.push((b'a' + rank - 1) as char);
            text.push('\n');
        }
        for color in Color::ALL {
            let mut line = String::new();
            for piece_type in HAND_TYPES {
                let count = self.hand(color, piece_type);
                if count > 0 {
                    line.push_str(&format!("{}{} ", piece_type.usi_char(), count));
                }
            }
            text.push_str(&format!(
                "{}: {}\n",
                match color {
                    Color::Black => "先手持駒",
                    Color::White => "後手持駒",
                },
                if line.is_empty() { "なし".into() } else { line }
            ));
        }
        text.push_str(&format!(
            "手番: {}\n",
            match self.side_to_move {
                Color::Black => "先手",
                Color::White => "後手",
            }
        ));
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hirate_round_trips_through_sfen() {
        let position = Position::hirate();
        assert_eq!(position.to_sfen(), HIRATE_SFEN);
    }

    #[test]
    fn hirate_has_forty_pieces_and_empty_hands() {
        let position = Position::hirate();
        assert_eq!(position.occupied().count(), 40);
        assert_eq!(position.pieces_of(Color::Black).count(), 20);
        assert_eq!(position.pieces_of(Color::White).count(), 20);
        for color in Color::ALL {
            for piece_type in HAND_TYPES {
                assert_eq!(position.hand(color, piece_type), 0);
            }
        }
    }

    #[test]
    fn hirate_puts_the_kings_where_they_belong() {
        let position = Position::hirate();
        assert_eq!(position.king_square(Color::Black), Some(Square::new(5, 9)));
        assert_eq!(position.king_square(Color::White), Some(Square::new(5, 1)));
    }

    #[test]
    fn nobody_is_in_check_at_the_start() {
        let position = Position::hirate();
        assert!(!position.is_in_check(Color::Black));
        assert!(!position.is_in_check(Color::White));
        assert!(!position.in_check());
    }

    #[test]
    fn hirate_is_symmetric_so_its_static_score_is_zero() {
        assert_eq!(Position::hirate().psq_score(), 0);
    }

    #[test]
    fn promoted_pieces_and_hands_round_trip() {
        // 持ち駒は「飛角金銀桂香歩・先手が先」の並びで書き戻すので、往復させるなら
        // 入力もその並びに揃えておく。
        let sfen = "l6nl/5+P1gk/2np1S3/p1p4Pp/3P2Sp1/1PPb2P1P/P5GS1/R8/LN4bKL w RGgsn5p 1";
        let position = Position::from_sfen(sfen).expect("読めるはず");
        assert_eq!(position.to_sfen(), sfen);
        assert_eq!(position.hand(Color::Black, PieceType::Gold), 1);
        assert_eq!(position.hand(Color::Black, PieceType::Rook), 1);
        assert_eq!(position.hand(Color::White, PieceType::Pawn), 5);
    }

    #[test]
    fn a_move_and_its_undo_restore_the_position_exactly() {
        let mut position = Position::hirate();
        let before = position.to_sfen();
        let key_before = position.key();
        let psq_before = position.psq_score();

        position.do_move(Move::from_usi("7g7f").unwrap());
        assert_ne!(position.to_sfen(), before);
        assert_ne!(position.key(), key_before);
        assert_eq!(position.side_to_move(), Color::White);

        position.undo_move();
        assert_eq!(position.to_sfen(), before);
        assert_eq!(position.key(), key_before, "鍵が戻っていない");
        assert_eq!(position.psq_score(), psq_before, "評価値が戻っていない");
    }

    #[test]
    fn the_incremental_key_matches_a_key_rebuilt_from_the_sfen() {
        let mut position = Position::hirate();
        for usi in ["7g7f", "3c3d", "8h2b+", "3a2b", "B*4e"] {
            position.do_move(Move::from_usi(usi).unwrap());
            let rebuilt = Position::from_sfen(&position.to_sfen()).unwrap();
            assert_eq!(
                position.key(),
                rebuilt.key(),
                "{usi} のあとで差分更新の鍵が作り直した鍵と食い違う"
            );
            assert_eq!(position.psq_score(), rebuilt.psq_score(), "{usi} のあとで評価値が食い違う");
        }
    }

    #[test]
    fn capturing_moves_a_promoted_piece_into_the_hand_unpromoted() {
        // 先手の飛車が 2 二 の後手の成銀を取って成る。
        let sfen = "4k4/7+s1/9/9/9/9/9/7R1/4K4 b - 1";
        let mut position = Position::from_sfen(sfen).expect("読めるはず");
        let before = position.to_sfen();
        position.do_move(Move::from_usi("2h2b+").unwrap());

        assert_eq!(
            position.hand(Color::Black, PieceType::Silver),
            1,
            "成銀は銀として持ち駒に入る"
        );
        assert_eq!(
            position.piece_at(Square::new(2, 2)).map(|p| p.piece_type),
            Some(PieceType::Dragon)
        );

        position.undo_move();
        assert_eq!(position.to_sfen(), before, "取った成銀は成銀のまま盤へ戻る");
    }

    #[test]
    fn dropping_and_undoing_restores_the_hand() {
        let sfen = "4k4/9/9/9/9/9/9/9/4K4 b P 1";
        let mut position = Position::from_sfen(sfen).expect("読めるはず");
        let before = position.to_sfen();
        let key_before = position.key();
        position.do_move(Move::from_usi("P*5e").unwrap());
        assert_eq!(position.hand(Color::Black, PieceType::Pawn), 0);
        position.undo_move();
        assert_eq!(position.to_sfen(), before);
        assert_eq!(position.key(), key_before);
    }

    #[test]
    fn a_rook_gives_check_along_the_file() {
        let position = Position::from_sfen("4k4/9/9/9/4R4/9/9/9/4K4 b - 1").unwrap();
        assert!(position.is_in_check(Color::White), "同じ 5 筋なので王手");

        let position = Position::from_sfen("4k4/9/9/9/9/9/9/9/3RK4 b - 1").unwrap();
        assert!(
            !position.is_in_check(Color::White),
            "6 筋の飛車は 5 筋の玉に当たらない"
        );
    }

    #[test]
    fn a_blocked_rook_gives_no_check() {
        let position = Position::from_sfen("4k4/4p4/9/9/4R4/9/9/9/4K4 b - 1").unwrap();
        assert!(!position.is_in_check(Color::White), "歩が遮っている");
    }

    #[test]
    fn a_horse_checks_with_its_king_step() {
        // 5 二 の馬は 5 一 の玉に「玉の 1 歩」で当たる（角の利きでは当たらない向き）。
        let position = Position::from_sfen("4k4/4+B4/9/9/9/9/9/9/4K4 b - 1").unwrap();
        assert!(position.is_in_check(Color::White));
    }

    #[test]
    fn the_cached_check_flag_agrees_with_a_fresh_computation() {
        let mut position = Position::hirate();
        for usi in ["7g7f", "3c3d", "8h2b+", "3a2b", "B*5e", "2b3c", "5e3c+"] {
            position.do_move(Move::from_usi(usi).unwrap());
            assert_eq!(
                position.in_check(),
                position.is_in_check(position.side_to_move()),
                "{usi} のあとでキャッシュした王手判定がずれた"
            );
        }
    }

    #[test]
    fn a_null_move_only_flips_the_side() {
        let mut position = Position::hirate();
        let before = position.key();
        position.do_null_move();
        assert_eq!(position.side_to_move(), Color::White);
        assert_ne!(position.key(), before);
        position.undo_null_move();
        assert_eq!(position.key(), before);
        assert_eq!(position.side_to_move(), Color::Black);
    }

    #[test]
    fn shuffling_pieces_back_and_forth_is_a_repetition_draw() {
        // 玉を往復させるだけの手順。4 回目の同一局面で千日手。
        let mut position =
            Position::from_sfen("4k4/9/9/9/9/9/9/9/4K4 b - 1").expect("読めるはず");
        assert_eq!(position.repetition(), Repetition::None);

        for _ in 0..3 {
            for usi in ["5i4i", "5a4a", "4i5i", "4a5a"] {
                position.do_move(Move::from_usi(usi).unwrap());
            }
        }
        assert_eq!(
            position.repetition(),
            Repetition::Draw,
            "同一局面 4 回で千日手"
        );
    }

    #[test]
    fn a_capture_resets_the_repetition_window() {
        let mut position = Position::hirate();
        position.do_move(Move::from_usi("7g7f").unwrap());
        position.do_move(Move::from_usi("3c3d").unwrap());
        position.do_move(Move::from_usi("8h2b+").unwrap());
        // 取る手が挟まったので、それ以前の局面とは照合しない。
        assert_eq!(position.repetition(), Repetition::None);
    }

    #[test]
    fn garbage_sfen_is_rejected() {
        assert!(Position::from_sfen("").is_err());
        assert!(Position::from_sfen("9/9/9/9/9/9/9/9 b - 1").is_err(), "段が 8 つ");
        assert!(Position::from_sfen("9/9/9/9/9/9/9/9/8 b - 1").is_err(), "筋が足りない");
        assert!(Position::from_sfen("9/9/9/9/9/9/9/9/9 x - 1").is_err(), "手番が不正");
        assert!(Position::from_sfen("9/9/9/9/9/9/9/9/+9 b - 1").is_err(), "`+` のあとが数字");
        assert!(Position::from_sfen("9/9/9/9/9/9/9/9/9 b K 1").is_err(), "玉は持ち駒にならない");
        assert!(
            Position::from_sfen("9/9/9/9/9/9/9/9/9 b 19P 1").is_err(),
            "歩は 18 枚まで"
        );
    }
}
