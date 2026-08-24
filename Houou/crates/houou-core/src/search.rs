//! 探索 — 反復深化 + PVS（alpha-beta）。
//!
//! # 骨格
//!
//! 1. **反復深化** — 深さ 1 から掘り直す。浅い探索の最善手が置換表に残るので、
//!    深い探索の並べ替えが良くなり、掘り直すコストより枝刈りの利得が上回る。
//!    途中で時間が尽きても**その時点で最も深く読めた手**が残るのが本質。
//! 2. **PVS** — 最初の手だけ全窓で読み、残りは「これより良くならない」ことだけを
//!    狭い窓で確かめる。外れたときだけ読み直す。
//! 3. **静止探索** — 葉で駒の取り合いが途中の局面を評価しないよう、取る手と成る手だけを
//!    読み切ってから評価する。これが無いと評価値が取り合いの途中で暴れる。
//!
//! # 打ち切り
//!
//! `stop_flag` と時間の両方を見る。深さ 1 は必ず読み切る（指す手が無いと困る）。

use crate::eval::{self, INFINITY, MATE, MATE_THRESHOLD};
use crate::movegen::{generate_legal_into, generate_noisy_into};
use crate::moves::{Move, MoveList, MAX_MOVES};
use crate::position::{Position, Repetition};
use crate::types::{Color, PieceType, NUM_PIECE_TYPES, NUM_SQUARES};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 読む深さの上限。これを超えると評価だけ返す。
pub const MAX_PLY: usize = 128;

/// 経過ミリ秒を返す時計。
///
/// **wasm32 に `std::time::Instant` は無い**（呼ぶと panic する）。時間制限は探索の
/// 中核なので、時計そのものを差し替えられるようにしてある。ネイティブでは既定の
/// `Instant` を使い、wasm ではホストが `Date.now()` を渡す（`Searcher::set_clock`）。
pub type Clock = fn() -> u64;

#[cfg(not(target_arch = "wasm32"))]
fn default_clock() -> u64 {
    use std::sync::LazyLock;
    use std::time::Instant;
    // プロセス起動時を原点にする。差だけ使うので原点はどこでも良い。
    static ORIGIN: LazyLock<Instant> = LazyLock::new(Instant::now);
    ORIGIN.elapsed().as_millis() as u64
}

/// wasm では時計を注入しないと時間制限が効かない。**気付かずに無限に読み続けるより、
/// 呼ばれた時点で落ちるほうが良い** — 差し替え忘れが本番で無限ループになるのを防ぐ。
#[cfg(target_arch = "wasm32")]
fn default_clock() -> u64 {
    panic!("wasm では Searcher::set_clock で時計を渡すこと（Instant が無い）");
}

/// 時間と停止要求をどれくらいの間隔で見るか。毎ノード見ると時計の読み取りが効く。
const CHECK_INTERVAL: u64 = 2048;

#[derive(Clone, Debug, Default)]
pub struct SearchLimits {
    /// 掘る深さの上限。
    pub depth: Option<u32>,
    /// 読むノード数の上限。
    pub nodes: Option<u64>,
    /// この 1 手に使ってよい時間。
    pub time_ms: Option<u64>,
}

impl SearchLimits {
    pub fn depth(depth: u32) -> SearchLimits {
        SearchLimits {
            depth: Some(depth),
            ..Default::default()
        }
    }

    pub fn time(time_ms: u64) -> SearchLimits {
        SearchLimits {
            time_ms: Some(time_ms),
            ..Default::default()
        }
    }
}

/// 1 回の反復が終わるたびに呼ぶ進捗。
#[derive(Clone, Debug)]
pub struct SearchInfo {
    pub depth: u32,
    pub score: i32,
    pub nodes: u64,
    pub elapsed_ms: u64,
    pub nps: u64,
    pub pv: Vec<Move>,
}

impl SearchInfo {
    /// 詰みが見えているなら「あと何手で詰むか」。手番側が詰ますなら正。
    pub fn mate_in(&self) -> Option<i32> {
        if self.score >= MATE_THRESHOLD {
            Some((MATE - self.score + 1) / 2)
        } else if self.score <= -MATE_THRESHOLD {
            Some(-((MATE + self.score + 1) / 2))
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SearchResult {
    pub best_move: Option<Move>,
    /// 相手の応手の読み（USI の `ponder`）。
    pub ponder_move: Option<Move>,
    pub score: i32,
    pub depth: u32,
    pub nodes: u64,
    pub elapsed_ms: u64,
}

// ---- 置換表 ------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Bound {
    /// 窓の中に収まった真の値。
    Exact,
    /// beta 以上で打ち切った。真の値はこれ以上。
    Lower,
    /// alpha を超えられなかった。真の値はこれ以下。
    Upper,
}

#[derive(Clone, Copy)]
struct Entry {
    key: u64,
    mv: Move,
    score: i32,
    depth: i16,
    bound: Bound,
    generation: u8,
}

impl Default for Entry {
    fn default() -> Entry {
        Entry {
            key: 0,
            mv: Move::default(),
            score: 0,
            depth: -1,
            bound: Bound::Exact,
            generation: 0,
        }
    }
}

struct TranspositionTable {
    entries: Vec<Entry>,
    mask: usize,
    generation: u8,
}

impl TranspositionTable {
    fn new(megabytes: usize) -> TranspositionTable {
        let entry_size = std::mem::size_of::<Entry>();
        let wanted = (megabytes.max(1) * 1024 * 1024) / entry_size;
        // 2 の冪に丸めると添字がマスクだけで済む。
        let count = wanted.next_power_of_two() / 2;
        let count = count.max(1024);
        TranspositionTable {
            entries: vec![Entry::default(); count],
            mask: count - 1,
            generation: 0,
        }
    }

    fn clear(&mut self) {
        self.entries.iter_mut().for_each(|e| *e = Entry::default());
        self.generation = 0;
    }

    #[inline]
    fn probe(&self, key: u64) -> Option<&Entry> {
        let entry = &self.entries[key as usize & self.mask];
        if entry.key == key && entry.depth >= 0 {
            Some(entry)
        } else {
            None
        }
    }

    #[inline]
    fn store(&mut self, key: u64, mv: Move, score: i32, depth: i32, bound: Bound) {
        let slot = &mut self.entries[key as usize & self.mask];
        // 深いほうを残す。ただし世代が変わっていれば浅くても上書きする
        // （前の対局の残骸を抱えたままにしない）。
        let stale = slot.generation != self.generation;
        if stale || slot.key == key || depth as i16 >= slot.depth {
            *slot = Entry {
                key,
                // 手が空なら前の手を残す。null window で失敗しただけの探索は手を持たない。
                mv: if mv == Move::default() && slot.key == key {
                    slot.mv
                } else {
                    mv
                },
                score,
                depth: depth as i16,
                bound,
                generation: self.generation,
            };
        }
    }
}

/// 詰みスコアは「根から何手で詰むか」で持ちたいが、置換表は違う深さで共有される。
/// 保存時に ply を抜き、取り出すときに足し直す。これを忘れると詰み手数が壊れる。
#[inline]
fn score_to_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_THRESHOLD {
        score + ply as i32
    } else if score <= -MATE_THRESHOLD {
        score - ply as i32
    } else {
        score
    }
}

#[inline]
fn score_from_tt(score: i32, ply: usize) -> i32 {
    if score >= MATE_THRESHOLD {
        score - ply as i32
    } else if score <= -MATE_THRESHOLD {
        score + ply as i32
    } else {
        score
    }
}

// ---- 探索器 ------------------------------------------------------------

pub struct Searcher {
    tt: TranspositionTable,
    /// beta 切りを起こした静かな手。同じ深さの兄弟局面でも効きやすい。
    killers: Box<[[Move; 2]; MAX_PLY]>,
    /// 「この駒をこのマスへ動かすと良かった」の累積。静かな手の並べ替えに使う。
    history: Box<[[[i32; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM]>,
    pv: Box<[[Move; MAX_PLY]; MAX_PLY]>,
    pv_len: [usize; MAX_PLY],
    stop: Arc<AtomicBool>,
    stopped: bool,
    nodes: u64,
    limits: SearchLimits,
    clock: Clock,
    start_ms: u64,
}

impl Searcher {
    pub fn new(tt_megabytes: usize) -> Searcher {
        Searcher {
            tt: TranspositionTable::new(tt_megabytes),
            killers: Box::new([[Move::default(); 2]; MAX_PLY]),
            history: Box::new([[[0; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM]),
            pv: Box::new([[Move::default(); MAX_PLY]; MAX_PLY]),
            pv_len: [0; MAX_PLY],
            stop: Arc::new(AtomicBool::new(false)),
            stopped: false,
            nodes: 0,
            limits: SearchLimits::default(),
            clock: default_clock,
            start_ms: 0,
        }
    }

    /// 外から探索を止めるための旗。**`search` はこれを勝手に下ろさない** —
    /// `go` の直前に呼ぶ側が下ろすこと（下ろす場所を探索側に持つと、
    /// `go` の直後に来た `stop` を取りこぼす）。
    pub fn stop_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.stop)
    }

    /// 時計を差し替える。wasm では**必ず**呼ぶこと（既定の時計は panic する）。
    pub fn set_clock(&mut self, clock: Clock) {
        self.clock = clock;
    }

    /// 対局をまたぐ状態を捨てる（USI の `usinewgame`）。
    pub fn clear(&mut self) {
        self.tt.clear();
        *self.killers = [[Move::default(); 2]; MAX_PLY];
        *self.history = [[[0; NUM_SQUARES]; NUM_PIECE_TYPES]; Color::NUM];
    }

    pub fn nodes(&self) -> u64 {
        self.nodes
    }

    /// 1 手考える。`on_info` は反復が 1 つ終わるたびに呼ばれる。
    pub fn search(
        &mut self,
        position: &mut Position,
        limits: &SearchLimits,
        on_info: &mut dyn FnMut(&SearchInfo),
    ) -> SearchResult {
        self.limits = limits.clone();
        self.start_ms = (self.clock)();
        self.nodes = 0;
        self.stopped = false;
        self.pv_len = [0; MAX_PLY];
        self.tt.generation = self.tt.generation.wrapping_add(1);
        // 前の探索の履歴は、値は活かしつつ影響を薄める。
        for color in self.history.iter_mut() {
            for piece in color.iter_mut() {
                for value in piece.iter_mut() {
                    *value /= 8;
                }
            }
        }

        let mut root_moves = MoveList::new();
        generate_legal_into(position, &mut root_moves);
        if root_moves.is_empty() {
            // 詰み。投了すべき局面。
            return SearchResult {
                best_move: None,
                score: -MATE,
                ..Default::default()
            };
        }

        let mut result = SearchResult {
            best_move: Some(root_moves[0]),
            ..Default::default()
        };

        let max_depth = limits.depth.unwrap_or(MAX_PLY as u32 - 2).min(MAX_PLY as u32 - 2);
        let mut previous = 0;

        for depth in 1..=max_depth {
            let score = self.search_root(position, depth as i32, previous);

            // 深さ 1 だけは必ず採る。指す手が無いまま帰るより、浅くても手がある方が良い。
            if self.stopped && depth > 1 {
                break;
            }

            previous = score;
            result.score = score;
            result.depth = depth;
            result.nodes = self.nodes;
            result.elapsed_ms = self.elapsed_ms();
            if self.pv_len[0] > 0 {
                result.best_move = Some(self.pv[0][0]);
                result.ponder_move = if self.pv_len[0] > 1 {
                    Some(self.pv[0][1])
                } else {
                    None
                };
            }

            let info = SearchInfo {
                depth,
                score,
                nodes: self.nodes,
                elapsed_ms: result.elapsed_ms,
                nps: self.nps(),
                pv: self.pv[0][..self.pv_len[0]].to_vec(),
            };
            on_info(&info);

            // 詰みが見えたらそれ以上掘る意味が無い。
            if score.abs() >= MATE_THRESHOLD {
                break;
            }
            // 次の反復が時間内に終わりそうになければ始めない。
            if let Some(budget) = self.limits.time_ms {
                if self.elapsed_ms() * 2 >= budget {
                    break;
                }
            }
            if self.stopped {
                break;
            }
        }

        result.nodes = self.nodes;
        result.elapsed_ms = self.elapsed_ms();
        result
    }

    /// 根の 1 反復。**アスピレーション窓**で狭く始め、外れたら広げ直す。
    ///
    /// 前の反復の値の近くに落ち着くことが多いので、狭い窓のほうが枝刈りが効く。
    /// 外れたときの読み直しのコストより、当たったときの利得が上回る。
    fn search_root(&mut self, position: &mut Position, depth: i32, previous: i32) -> i32 {
        if depth < 5 || previous.abs() >= MATE_THRESHOLD {
            return self.alpha_beta(position, depth, -INFINITY, INFINITY, 0);
        }

        let mut window = 24;
        loop {
            let alpha = (previous - window).max(-INFINITY);
            let beta = (previous + window).min(INFINITY);
            let score = self.alpha_beta(position, depth, alpha, beta, 0);
            if self.stopped {
                return score;
            }
            if score > alpha && score < beta {
                return score;
            }
            window *= 3;
            if window > 1200 {
                return self.alpha_beta(position, depth, -INFINITY, INFINITY, 0);
            }
        }
    }

    fn alpha_beta(
        &mut self,
        position: &mut Position,
        mut depth: i32,
        mut alpha: i32,
        mut beta: i32,
        ply: usize,
    ) -> i32 {
        self.pv_len[ply] = 0;
        let is_root = ply == 0;
        let is_pv = beta - alpha > 1;

        if !is_root {
            match position.repetition() {
                Repetition::Draw => return 0,
                Repetition::Win => return MATE - ply as i32,
                Repetition::Loss => return -(MATE - ply as i32),
                Repetition::None => {}
            }

            // 詰み距離枝刈り — すでに見つけた詰みより遠い詰みは調べる価値が無い。
            alpha = alpha.max(-(MATE - ply as i32));
            beta = beta.min(MATE - ply as i32 - 1);
            if alpha >= beta {
                return alpha;
            }
        }

        if depth <= 0 {
            return self.quiescence(position, alpha, beta, ply);
        }

        self.nodes += 1;
        if self.should_stop() {
            self.stopped = true;
            return 0;
        }
        if ply >= MAX_PLY - 1 {
            return eval::evaluate(position);
        }

        let in_check = position.in_check();
        // 王手は 1 手延長する。王手は選択肢が狭く、読み違いが致命的になりやすい。
        if in_check {
            depth += 1;
        }

        let key = position.key();
        let mut tt_move = Move::default();
        if let Some(entry) = self.tt.probe(key) {
            tt_move = entry.mv;
            if !is_pv && entry.depth as i32 >= depth {
                let score = score_from_tt(entry.score, ply);
                let usable = match entry.bound {
                    Bound::Exact => true,
                    Bound::Lower => score >= beta,
                    Bound::Upper => score <= alpha,
                };
                if usable {
                    return score;
                }
            }
        }

        let static_eval = if in_check {
            -INFINITY
        } else {
            eval::evaluate(position)
        };

        // 手を渡しても beta を割らないなら、この枝は読むまでもなく良すぎる。
        // 王手中と、駒を渡すと動けなくなる局面（zugzwang）では使えない。
        if !is_pv
            && !in_check
            && depth >= 3
            && static_eval >= beta
            && self.has_material_to_spare(position)
        {
            let reduction = 2 + depth / 6;
            position.do_null_move();
            let score = -self.alpha_beta(position, depth - 1 - reduction, -beta, -beta + 1, ply + 1);
            position.undo_null_move();
            if self.stopped {
                return 0;
            }
            if score >= beta {
                // 詰みスコアは信用しない（パスで詰みが出るのは幻）。
                return if score >= MATE_THRESHOLD { beta } else { score };
            }
        }

        let mut moves = MoveList::new();
        generate_legal_into(position, &mut moves);
        if moves.is_empty() {
            // 将棋にステイルメイトは無い。合法手が無い = 詰み。
            return -(MATE - ply as i32);
        }

        let mut order = [0i32; MAX_MOVES];
        for index in 0..moves.len() {
            order[index] = self.score_move(position, moves[index], tt_move, ply);
        }

        let mut best_score = -INFINITY;
        let mut best_move = Move::default();
        let mut bound = Bound::Upper;

        for index in 0..moves.len() {
            // 全部並べ替えず、必要になった順に最大を 1 つ取り出す。
            // beta 切りで途中で抜けることが多いので、そのほうが安い。
            let mut best_index = index;
            for candidate in index + 1..moves.len() {
                if order[candidate] > order[best_index] {
                    best_index = candidate;
                }
            }
            moves.swap(index, best_index);
            order.swap(index, best_index);

            let mv = moves[index];
            let is_capture = position.piece_at(mv.to()).is_some();
            let is_quiet = !is_capture && !mv.is_promotion();

            position.do_move(mv);
            let gives_check = position.in_check();

            let score = if index == 0 {
                -self.alpha_beta(position, depth - 1, -beta, -alpha, ply + 1)
            } else {
                // 後ろのほうの静かな手は浅く読む（LMR）。並べ替えが当たっていれば
                // そこに最善手は無いので、浅く見て alpha を超えなければ捨ててよい。
                let reduction = if depth >= 3 && index >= 3 && is_quiet && !in_check && !gives_check
                {
                    let base = 1 + (depth / 8).min(2) + (index / 12).min(2) as i32;
                    if is_pv {
                        (base - 1).max(0)
                    } else {
                        base
                    }
                } else {
                    0
                };

                let mut score =
                    -self.alpha_beta(position, depth - 1 - reduction, -alpha - 1, -alpha, ply + 1);
                // 浅く読んで超えてきたら、削らずに読み直す。
                if score > alpha && reduction > 0 {
                    score = -self.alpha_beta(position, depth - 1, -alpha - 1, -alpha, ply + 1);
                }
                // 窓の中に入ったら全窓で確かめる。
                if score > alpha && score < beta {
                    score = -self.alpha_beta(position, depth - 1, -beta, -alpha, ply + 1);
                }
                score
            };

            position.undo_move();

            if self.stopped {
                return 0;
            }

            if score > best_score {
                best_score = score;
                best_move = mv;

                if score > alpha {
                    alpha = score;
                    bound = Bound::Exact;
                    self.update_pv(ply, mv);

                    if alpha >= beta {
                        bound = Bound::Lower;
                        if is_quiet {
                            self.remember_killer(ply, mv);
                            self.bump_history(position, mv, depth);
                        }
                        break;
                    }
                }
            }
        }

        self.tt
            .store(key, best_move, score_to_tt(best_score, ply), depth, bound);
        best_score
    }

    /// 静止探索 — 取り合いが途中の局面を評価しないための読み切り。
    fn quiescence(
        &mut self,
        position: &mut Position,
        mut alpha: i32,
        beta: i32,
        ply: usize,
    ) -> i32 {
        self.nodes += 1;
        if self.should_stop() {
            self.stopped = true;
            return 0;
        }
        if ply >= MAX_PLY - 1 {
            return eval::evaluate(position);
        }

        let in_check = position.in_check();
        let mut best_score = -INFINITY;
        let mut stand_pat = 0;

        if !in_check {
            // 「何もしない」権利。取り合いに応じない選択肢が常にあるので下限になる。
            stand_pat = eval::evaluate(position);
            best_score = stand_pat;
            if stand_pat >= beta {
                return stand_pat;
            }
            if stand_pat > alpha {
                alpha = stand_pat;
            }
        }

        let mut moves = MoveList::new();
        if in_check {
            // 王手されているなら「取る手だけ」では逃げ道を見落とす。全部読む。
            generate_legal_into(position, &mut moves);
            if moves.is_empty() {
                return -(MATE - ply as i32);
            }
        } else {
            generate_noisy_into(position, &mut moves);
        }

        let mut order = [0i32; MAX_MOVES];
        for index in 0..moves.len() {
            order[index] = self.score_move(position, moves[index], Move::default(), ply);
        }

        for index in 0..moves.len() {
            let mut best_index = index;
            for candidate in index + 1..moves.len() {
                if order[candidate] > order[best_index] {
                    best_index = candidate;
                }
            }
            moves.swap(index, best_index);
            order.swap(index, best_index);
            let mv = moves[index];

            // 取っても届かない手は読まない（デルタ枝刈り）。
            if !in_check && best_score > -MATE_THRESHOLD {
                let optimistic = eval::capture_gain(position, mv.to())
                    + if mv.is_promotion() { 400 } else { 0 };
                if stand_pat + optimistic + 200 < alpha {
                    continue;
                }
            }

            position.do_move(mv);
            let score = -self.quiescence(position, -beta, -alpha, ply + 1);
            position.undo_move();

            if self.stopped {
                return 0;
            }
            if score > best_score {
                best_score = score;
                if score > alpha {
                    alpha = score;
                    if alpha >= beta {
                        break;
                    }
                }
            }
        }

        best_score
    }

    // ---- 並べ替え ------------------------------------------------------

    /// 手の見込み。大きいほど先に読む。
    ///
    /// 良い手を先に読むほど beta 切りが早く起き、読むノードが減る。**探索の速さは
    /// ここの質でほぼ決まる** — 枝刈りの工夫より並べ替えのほうが効く。
    fn score_move(&self, position: &Position, mv: Move, tt_move: Move, ply: usize) -> i32 {
        if mv == tt_move && tt_move != Move::default() {
            return 1_000_000;
        }

        let victim = position.piece_at(mv.to());
        if let Some(victim) = victim {
            // MVV-LVA — 高い駒を安い駒で取る手から読む。
            let attacker = self
                .moved_piece(position, mv)
                .map(eval::material)
                .unwrap_or(0);
            let gain = eval::material(victim.piece_type) * 16 - attacker;
            return 500_000 + gain + if mv.is_promotion() { 2_000 } else { 0 };
        }

        if mv.is_promotion() {
            return 400_000;
        }

        if ply < MAX_PLY {
            if mv == self.killers[ply][0] {
                return 300_000;
            }
            if mv == self.killers[ply][1] {
                return 290_000;
            }
        }

        // 打つ手は静かな手のなかでは働きが読みにくい。少しだけ後ろに置く。
        let drop_penalty = if mv.is_drop() { 1_000 } else { 0 };
        self.history_of(position, mv) - drop_penalty
    }

    /// 動かす駒の種類。打ちなら打つ駒。
    #[inline]
    fn moved_piece(&self, position: &Position, mv: Move) -> Option<PieceType> {
        match mv.dropped_piece() {
            Some(piece_type) => Some(piece_type),
            None => mv
                .from()
                .and_then(|from| position.piece_at(from))
                .map(|piece| piece.piece_type),
        }
    }

    #[inline]
    fn history_of(&self, position: &Position, mv: Move) -> i32 {
        match self.moved_piece(position, mv) {
            Some(piece_type) => {
                self.history[position.side_to_move().index()][piece_type.index()]
                    [mv.to().index()]
            }
            None => 0,
        }
    }

    fn bump_history(&mut self, position: &Position, mv: Move, depth: i32) {
        let Some(piece_type) = self.moved_piece(position, mv) else {
            return;
        };
        // undo 済みなので手番は指した側に戻っている。
        let color = position.side_to_move().index();
        let slot = &mut self.history[color][piece_type.index()][mv.to().index()];
        *slot += depth * depth;
        // 上限を設けないと、序盤の一手が延々と首位を占め続ける。
        if *slot > 1 << 20 {
            for piece in self.history[color].iter_mut() {
                for value in piece.iter_mut() {
                    *value /= 2;
                }
            }
        }
    }

    fn remember_killer(&mut self, ply: usize, mv: Move) {
        if ply >= MAX_PLY {
            return;
        }
        if self.killers[ply][0] != mv {
            self.killers[ply][1] = self.killers[ply][0];
            self.killers[ply][0] = mv;
        }
    }

    fn update_pv(&mut self, ply: usize, mv: Move) {
        self.pv[ply][0] = mv;
        let child_len = if ply + 1 < MAX_PLY {
            self.pv_len[ply + 1]
        } else {
            0
        };
        for index in 0..child_len {
            self.pv[ply][index + 1] = self.pv[ply + 1][index];
        }
        self.pv_len[ply] = child_len + 1;
    }

    // ---- 打ち切りと時計 ------------------------------------------------

    /// パスしても大丈夫か。歩と玉しか無いと、手を渡すこと自体が損になる局面
    /// （zugzwang）が起きやすく、null move の前提が崩れる。
    fn has_material_to_spare(&self, position: &Position) -> bool {
        let us = position.side_to_move();
        // 盤上でも持ち駒でも数える駒。`hand` は持ち駒になり得る駒種しか受け取らない。
        for piece_type in [
            PieceType::Rook,
            PieceType::Bishop,
            PieceType::Gold,
            PieceType::Silver,
            PieceType::Knight,
            PieceType::Lance,
        ] {
            if position.pieces(us, piece_type).is_not_empty() || position.hand(us, piece_type) > 0 {
                return true;
            }
        }
        // 成駒は盤上にしか居ない（取れば成る前に戻る）。
        for piece_type in [
            PieceType::Dragon,
            PieceType::Horse,
            PieceType::ProSilver,
            PieceType::ProKnight,
            PieceType::ProLance,
        ] {
            if position.pieces(us, piece_type).is_not_empty() {
                return true;
            }
        }
        false
    }

    fn should_stop(&mut self) -> bool {
        if self.stopped {
            return true;
        }
        if let Some(limit) = self.limits.nodes {
            if self.nodes >= limit {
                return true;
            }
        }
        // 停止要求は毎ノード見る。relaxed な atomic の読みは十分安く、
        // ここを間引くと `stop` に最大 2048 ノードぶん反応が遅れる。
        if self.stop.load(Ordering::Relaxed) {
            return true;
        }
        // 時計の読み取りだけは間引く。
        if !self.nodes.is_multiple_of(CHECK_INTERVAL) {
            return false;
        }
        match self.limits.time_ms {
            Some(budget) => self.elapsed_ms() >= budget,
            None => false,
        }
    }

    fn elapsed_ms(&self) -> u64 {
        (self.clock)().saturating_sub(self.start_ms)
    }

    fn nps(&self) -> u64 {
        // 1ms 未満は 0 で割ることになるので、そのときは測れないものとして 0 を返す。
        (self.nodes * 1_000)
            .checked_div(self.elapsed_ms())
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn search_sfen(sfen: &str, limits: SearchLimits) -> (SearchResult, Position) {
        let mut position = Position::from_sfen(sfen).expect("読めるはず");
        let mut searcher = Searcher::new(16);
        let result = searcher.search(&mut position, &limits, &mut |_| {});
        (result, position)
    }

    #[test]
    fn it_always_returns_a_legal_move() {
        let (result, mut position) =
            search_sfen(crate::position::HIRATE_SFEN, SearchLimits::depth(4));
        let mv = result.best_move.expect("平手で指す手が無いはずがない");
        assert!(
            crate::movegen::is_legal_move(&mut position, mv),
            "{} は合法手ではない",
            mv.to_usi()
        );
    }

    #[test]
    fn it_finds_a_mate_in_one() {
        // 1 一 の後手玉。金を **2 二** に打つのが詰み — 玉の逃げ道 1 二・2 一 を金が抑え、
        // 金自身は 2 九 の飛車が支えている。1 二 への金打ちは玉に取られるので詰みではない。
        let (result, _) = search_sfen("8k/9/9/9/9/9/9/9/K6R1 b G 1", SearchLimits::depth(3));
        assert_eq!(
            result.best_move.map(|mv| mv.to_usi()),
            Some("G*2b".to_string()),
            "詰みを見つけていない"
        );
        assert!(result.score >= MATE_THRESHOLD, "詰みのスコアになっていない");
    }

    #[test]
    fn it_takes_a_free_rook() {
        // 先手の飛車がただで取れる位置に後手の飛車が居る。
        let (result, _) = search_sfen("4k4/9/9/9/4r4/9/9/4R4/4K4 b - 1", SearchLimits::depth(6));
        let best = result.best_move.expect("手がある").to_usi();
        assert_eq!(best, "5h5e", "ただの飛車を取っていない: {best}");
    }

    #[test]
    fn it_does_not_hang_a_piece_for_nothing() {
        // 先手の銀は 5 六、後手の歩は 5 四。銀が 5 五 へ出ると歩にただで取られる。
        // 5 五 は空きマスなので、取り返しも無い純粋な駒損。
        let (result, _) = search_sfen("4k4/9/9/4p4/9/4S4/9/9/4K4 b - 1", SearchLimits::depth(6));
        let best = result.best_move.expect("手がある").to_usi();
        assert_ne!(best, "5f5e", "歩に取られるマスへ銀を出している");
    }

    #[test]
    fn deeper_search_reads_more_nodes() {
        let mut position = Position::hirate();
        let mut searcher = Searcher::new(16);
        let shallow = searcher.search(&mut position, &SearchLimits::depth(3), &mut |_| {});
        let mut searcher = Searcher::new(16);
        let deep = searcher.search(&mut position, &SearchLimits::depth(6), &mut |_| {});
        assert!(deep.nodes > shallow.nodes);
        assert_eq!(deep.depth, 6);
    }

    #[test]
    fn the_search_leaves_the_position_untouched() {
        let mut position = Position::hirate();
        let before = position.to_sfen();
        let key_before = position.key();
        let mut searcher = Searcher::new(16);
        searcher.search(&mut position, &SearchLimits::depth(5), &mut |_| {});
        assert_eq!(position.to_sfen(), before);
        assert_eq!(position.key(), key_before);
        assert_eq!(position.history_len(), 0);
    }

    #[test]
    fn a_time_limit_is_respected() {
        let mut position = Position::hirate();
        let mut searcher = Searcher::new(16);
        let start = Instant::now();
        let result = searcher.search(&mut position, &SearchLimits::time(200), &mut |_| {});
        let elapsed = start.elapsed().as_millis() as u64;
        assert!(result.best_move.is_some());
        assert!(elapsed < 1_000, "200ms の予算で {elapsed}ms 使った");
    }

    #[test]
    fn the_stop_flag_cuts_the_search_short() {
        let mut position = Position::hirate();
        let mut searcher = Searcher::new(16);
        let flag = searcher.stop_flag();
        flag.store(true, Ordering::Relaxed);
        let result = searcher.search(&mut position, &SearchLimits::depth(30), &mut |_| {});
        assert!(result.best_move.is_some(), "止めても指す手は返す");
        // 深さ 1 だけは読み切ってから止まる（指す手が無いまま帰らせない）。
        assert_eq!(result.depth, 1, "止まっていない");
    }

    #[test]
    fn a_mated_position_reports_no_move() {
        // 後手玉が詰んでいる局面で後手番。
        let (result, _) = search_sfen("7Rk/7G1/9/9/9/9/9/9/K8 w - 1", SearchLimits::depth(2));
        assert_eq!(result.best_move, None);
        assert_eq!(result.score, -MATE);
    }

    #[test]
    fn the_reported_pv_is_playable() {
        let mut position = Position::hirate();
        let mut searcher = Searcher::new(16);
        let mut last: Vec<Move> = Vec::new();
        searcher.search(&mut position, &SearchLimits::depth(6), &mut |info| {
            last = info.pv.clone();
        });
        assert!(!last.is_empty(), "PV が空");
        let mut played = 0;
        for mv in &last {
            assert!(
                crate::movegen::is_legal_move(&mut position, *mv),
                "PV の {} 手目 {} が合法手ではない",
                played + 1,
                mv.to_usi()
            );
            position.do_move(*mv);
            played += 1;
        }
        for _ in 0..played {
            position.undo_move();
        }
    }
}
