//! 盤の語彙 — 手番・マス・駒種・駒。
//!
//! # 駒種の並びには意味がある
//!
//! `Pawn..=Rook` を 0..=5、`Gold`/`King` を 6/7、成駒を 8..=13 に置いてある。この並びのおかげで
//!
//! - **成りが `+8`**（`Pawn(0) → ProPawn(8)`, `Rook(5) → Dragon(13)`）。成れない駒だけが
//!   `+8` の範囲から外れる（`Gold`/`King`）ので、成れるかどうかが `pt <= Rook` で判る。
//! - **持ち駒になり得る駒種が `Pawn..=Gold` の 0..=6 で連続**する。持ち駒配列を駒種で
//!   そのまま添字できる。
//!
//! 並べ替えると両方が同時に壊れるので、変えるときは `promoted` / `HAND_TYPES` の両方を見ること。

/// 盤のマス数。
pub const NUM_SQUARES: usize = 81;

/// 手番。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum Color {
    /// 先手。盤の下側に居て、段が減る向きへ進む。
    Black = 0,
    /// 後手。
    White = 1,
}

impl Color {
    pub const NUM: usize = 2;
    pub const ALL: [Color; 2] = [Color::Black, Color::White];

    #[inline]
    pub const fn flip(self) -> Color {
        match self {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    /// 前進で段がどちら向きに動くか。先手は 1 段目へ向かうので負。
    #[inline]
    pub const fn forward(self) -> i8 {
        match self {
            Color::Black => -1,
            Color::White => 1,
        }
    }

    /// 敵陣（成れる段）に入っているか。先手は 1〜3 段目、後手は 7〜9 段目。
    #[inline]
    pub const fn is_promotion_rank(self, rank: u8) -> bool {
        match self {
            Color::Black => rank <= 3,
            Color::White => rank >= 7,
        }
    }
}

/// マス。`index = (筋 - 1) * 9 + (段 - 1)` の筋優先で並べる（1一 が 0、1二 が 1、2一 が 9）。
///
/// 筋は将棋の慣習どおり右から 1〜9、段は上から 1〜9。USI の `7g` は筋 7・段 7。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub struct Square(u8);

impl Square {
    /// 筋・段（ともに 1..=9）から作る。
    ///
    /// # Panics
    /// 範囲外なら panic する。盤外は `Option` ではなく**呼ぶ前に弾く**方針
    /// （生成器の内側で毎手 `Option` を剥がすのを避けるため）。
    #[inline]
    pub const fn new(file: u8, rank: u8) -> Square {
        assert!(file >= 1 && file <= 9 && rank >= 1 && rank <= 9);
        Square((file - 1) * 9 + (rank - 1))
    }

    /// 筋・段が盤内なら作る。盤外なら `None`。テーブル構築で使う。
    #[inline]
    pub const fn try_new(file: i8, rank: i8) -> Option<Square> {
        if file >= 1 && file <= 9 && rank >= 1 && rank <= 9 {
            Some(Square(((file - 1) * 9 + (rank - 1)) as u8))
        } else {
            None
        }
    }

    #[inline]
    pub const fn from_index(index: u8) -> Square {
        assert!((index as usize) < NUM_SQUARES);
        Square(index)
    }

    #[inline]
    pub const fn index(self) -> usize {
        self.0 as usize
    }

    #[inline]
    pub const fn file(self) -> u8 {
        self.0 / 9 + 1
    }

    #[inline]
    pub const fn rank(self) -> u8 {
        self.0 % 9 + 1
    }

    /// USI のマス表記（`7g` など）。
    pub fn to_usi(self) -> String {
        let mut s = String::with_capacity(2);
        s.push((b'0' + self.file()) as char);
        s.push((b'a' + self.rank() - 1) as char);
        s
    }

    /// USI のマス表記を読む。
    pub fn from_usi(text: &str) -> Option<Square> {
        let bytes = text.as_bytes();
        if bytes.len() != 2 {
            return None;
        }
        let file = bytes[0].checked_sub(b'0')?;
        let rank = bytes[1].checked_sub(b'a')? + 1;
        Square::try_new(file as i8, rank as i8)
    }

    /// 全マスを 0..81 の順に。
    pub fn all() -> impl Iterator<Item = Square> {
        (0..NUM_SQUARES as u8).map(Square)
    }
}

/// 駒種。並びの意味は module の説明を見ること。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
pub enum PieceType {
    Pawn = 0,
    Lance = 1,
    Knight = 2,
    Silver = 3,
    Bishop = 4,
    Rook = 5,
    Gold = 6,
    King = 7,
    ProPawn = 8,
    ProLance = 9,
    ProKnight = 10,
    ProSilver = 11,
    Horse = 12,
    Dragon = 13,
}

/// 駒種の総数（成駒を含む）。
pub const NUM_PIECE_TYPES: usize = 14;

/// 持ち駒になり得る駒種。`Pawn..=Gold` が連続しているので添字にそのまま使える。
pub const HAND_TYPES: [PieceType; 7] = [
    PieceType::Pawn,
    PieceType::Lance,
    PieceType::Knight,
    PieceType::Silver,
    PieceType::Bishop,
    PieceType::Rook,
    PieceType::Gold,
];

/// 持ち駒の種類数。
pub const NUM_HAND_TYPES: usize = 7;

impl PieceType {
    pub const ALL: [PieceType; NUM_PIECE_TYPES] = [
        PieceType::Pawn,
        PieceType::Lance,
        PieceType::Knight,
        PieceType::Silver,
        PieceType::Bishop,
        PieceType::Rook,
        PieceType::Gold,
        PieceType::King,
        PieceType::ProPawn,
        PieceType::ProLance,
        PieceType::ProKnight,
        PieceType::ProSilver,
        PieceType::Horse,
        PieceType::Dragon,
    ];

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }

    #[inline]
    pub const fn from_index(index: u8) -> PieceType {
        assert!((index as usize) < NUM_PIECE_TYPES);
        // 並びは `ALL` と一致しているので、添字から復元できる。
        PieceType::ALL[index as usize]
    }

    #[inline]
    pub const fn is_promoted(self) -> bool {
        (self as u8) >= 8
    }

    /// 成れる駒か。`Gold` と `King`、および成駒は成れない。
    #[inline]
    pub const fn can_promote(self) -> bool {
        (self as u8) <= (PieceType::Rook as u8)
    }

    /// 成った駒種。成れない駒なら `None`。
    #[inline]
    pub const fn promoted(self) -> Option<PieceType> {
        if self.can_promote() {
            Some(PieceType::from_index(self as u8 + 8))
        } else {
            None
        }
    }

    /// 成る前の駒種。成っていなければそのまま。駒を取ったとき持ち駒へ戻す先。
    #[inline]
    pub const fn unpromoted(self) -> PieceType {
        if self.is_promoted() {
            PieceType::from_index(self as u8 - 8)
        } else {
            self
        }
    }

    /// 金と同じ動きをするか（金・と・成香・成桂・成銀）。
    #[inline]
    pub const fn moves_like_gold(self) -> bool {
        matches!(
            self,
            PieceType::Gold
                | PieceType::ProPawn
                | PieceType::ProLance
                | PieceType::ProKnight
                | PieceType::ProSilver
        )
    }

    /// SFEN / USI で使う 1 文字（成駒は `+` を前置した 2 文字になる）。大文字は先手。
    pub const fn usi_char(self) -> char {
        match self.unpromoted() {
            PieceType::Pawn => 'P',
            PieceType::Lance => 'L',
            PieceType::Knight => 'N',
            PieceType::Silver => 'S',
            PieceType::Bishop => 'B',
            PieceType::Rook => 'R',
            PieceType::Gold => 'G',
            PieceType::King => 'K',
            // `unpromoted()` を通しているのでここには来ない。
            _ => '?',
        }
    }

    /// SFEN の 1 文字（大文字・成りなし）から駒種を読む。
    pub const fn from_usi_char(c: char) -> Option<PieceType> {
        match c {
            'P' => Some(PieceType::Pawn),
            'L' => Some(PieceType::Lance),
            'N' => Some(PieceType::Knight),
            'S' => Some(PieceType::Silver),
            'B' => Some(PieceType::Bishop),
            'R' => Some(PieceType::Rook),
            'G' => Some(PieceType::Gold),
            'K' => Some(PieceType::King),
            _ => None,
        }
    }

    /// その色の駒がこの段に居ると二度と動けなくなるか（行き所のない駒）。
    ///
    /// 歩・香は最終段、桂は最終 2 段。移動でも打ちでも同じ判定を使う。
    #[inline]
    pub const fn is_dead_at(self, color: Color, rank: u8) -> bool {
        let from_far_edge = match color {
            Color::Black => rank,
            Color::White => 10 - rank,
        };
        match self {
            PieceType::Pawn | PieceType::Lance => from_far_edge <= 1,
            PieceType::Knight => from_far_edge <= 2,
            _ => false,
        }
    }
}

/// 盤上の駒 — 色と駒種の組。
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Piece {
    pub color: Color,
    pub piece_type: PieceType,
}

impl Piece {
    #[inline]
    pub const fn new(color: Color, piece_type: PieceType) -> Piece {
        Piece { color, piece_type }
    }

    /// SFEN の駒表記（先手は大文字、成駒は `+` 前置）。
    pub fn to_sfen(self) -> String {
        let mut s = String::with_capacity(2);
        if self.piece_type.is_promoted() {
            s.push('+');
        }
        let c = self.piece_type.usi_char();
        s.push(match self.color {
            Color::Black => c,
            Color::White => c.to_ascii_lowercase(),
        });
        s
    }
}
