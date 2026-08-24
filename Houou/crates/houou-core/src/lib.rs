//! Houou — 将棋エンジンの中核。
//!
//! # この crate の役割
//!
//! 盤・合法手生成・（後で）探索と評価。**依存ゼロ**を保つ。理由は行き先が 2 つあるため:
//!
//! - `houou-wasm` 経由で Web（`Tsubame/examples/shogi-demo` の `ShogiEngine` プラグイン）
//! - Torimi の Android Rust ホストへ staticlib としてリンクし、埋め込み Hermes へ
//!   ホスト注入グローバルとして生やす（Hermes には `WebAssembly` も `Worker` も無いので、
//!   ネイティブで強いエンジンを動かせる経路はこれだけ）
//!
//! # 今どこまで出来ているか
//!
//! Phase 0（盤・合法手生成・perft）まで。探索・評価・NNUE はまだ無い。
//! 合法手生成の正しさは [`perft`] と、`Tsubame/examples/shogi-demo` 側の tsshogi
//! 差分テストの 2 本で押さえる。

pub mod attacks;
pub mod bitboard;
pub mod eval;
pub mod movegen;
pub mod moves;
pub mod perft;
pub mod position;
pub mod search;
pub mod types;
pub mod zobrist;

pub use bitboard::Bitboard;
pub use eval::evaluate;
pub use search::{SearchInfo, SearchLimits, SearchResult, Searcher};
pub use movegen::{generate_legal, generate_legal_into, is_checkmate, is_legal_move};
pub use moves::{Move, MoveList};
pub use perft::{perft, perft_divide};
pub use position::{Position, Repetition, SfenError, HIRATE_SFEN};
pub use types::{Color, Piece, PieceType, Square};
