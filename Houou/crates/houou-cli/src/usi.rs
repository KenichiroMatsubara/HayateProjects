//! USI プロトコル。将棋所 / ShogiHome から対局でき、エンジン同士を自動対戦させられる。
//!
//! **強さの測定はここに乗る。** 評価や探索の変更が本当に強くしたのかは、自己対局の
//! 勝率でしか判らない。プロトコルを早く用意しておくのはそのため。
//!
//! # スレッドの分担
//!
//! `go` で探索スレッドを起こし、**本体は標準入力を読み続ける**。`stop` は共有の
//! 停止旗を立てるだけ。探索は毎ノードその旗を見るので、待たずに止まる。
//! 探索結果（`bestmove`）は探索スレッド自身が書く。

use houou_core::search::{SearchInfo, SearchLimits, SearchResult, Searcher};
use houou_core::{Color, Position, HIRATE_SFEN};
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

const ENGINE_NAME: &str = concat!("Houou ", env!("CARGO_PKG_VERSION"));
const ENGINE_AUTHOR: &str = "KenichiroMatsubara";

/// 置換表の既定サイズ（MB）。
const DEFAULT_HASH_MB: usize = 128;

/// `go` に付いてきた条件。
#[derive(Debug, Default, Clone)]
struct GoParams {
    btime: Option<u64>,
    wtime: Option<u64>,
    binc: Option<u64>,
    winc: Option<u64>,
    byoyomi: Option<u64>,
    movetime: Option<u64>,
    depth: Option<u32>,
    nodes: Option<u64>,
    infinite: bool,
}

impl GoParams {
    fn parse(tokens: &[&str]) -> GoParams {
        /// `<キーワード> <値>` の値を読む。`depth` だけ `u32` なので型引数で受ける。
        fn value<T: std::str::FromStr>(tokens: &[&str], index: usize) -> Option<T> {
            tokens.get(index + 1).and_then(|t| t.parse().ok())
        }

        let mut params = GoParams::default();
        let mut index = 0;
        while index < tokens.len() {
            match tokens[index] {
                "btime" => params.btime = value(tokens, index),
                "wtime" => params.wtime = value(tokens, index),
                "binc" => params.binc = value(tokens, index),
                "winc" => params.winc = value(tokens, index),
                "byoyomi" => params.byoyomi = value(tokens, index),
                "movetime" => params.movetime = value(tokens, index),
                "depth" => params.depth = value(tokens, index),
                "nodes" => params.nodes = value(tokens, index),
                "infinite" => {
                    params.infinite = true;
                    index += 1;
                    continue;
                }
                _ => {
                    index += 1;
                    continue;
                }
            }
            index += 2;
        }
        params
    }

    /// この 1 手に使ってよい時間。`None` は無制限（`stop` を待つ）。
    ///
    /// 持ち時間の使い方は素朴に「残りの 1/40 + 加算のほとんど + 秒読み全部」。
    /// 秒読みがあるなら毎手それを使い切ってよく、無いなら残りを 40 手ぶんに割る。
    fn budget_ms(&self, side: Color) -> Option<u64> {
        if self.infinite {
            return None;
        }
        if let Some(movetime) = self.movetime {
            return Some(movetime);
        }
        if self.depth.is_some() || self.nodes.is_some() {
            return None;
        }

        let (remaining, increment) = match side {
            Color::Black => (self.btime, self.binc),
            Color::White => (self.wtime, self.winc),
        };
        let remaining = remaining.unwrap_or(0);
        let increment = increment.unwrap_or(0);
        let byoyomi = self.byoyomi.unwrap_or(0);

        if remaining == 0 && byoyomi == 0 && increment == 0 {
            return None;
        }

        let base = remaining / 40 + increment * 4 / 5 + byoyomi;
        // 通信と出力のぶんを引く。切れ負けは何より避ける。
        Some(base.saturating_sub(80).max(20))
    }

    fn limits(&self, side: Color) -> SearchLimits {
        SearchLimits {
            depth: self.depth,
            nodes: self.nodes,
            time_ms: self.budget_ms(side),
        }
    }
}

/// 探索中のスレッドと、それを止める旗。
struct Running {
    handle: JoinHandle<()>,
    stop: Arc<AtomicBool>,
}

pub struct UsiEngine {
    position: Position,
    searcher: Arc<Mutex<Searcher>>,
    running: Option<Running>,
    hash_mb: usize,
}

impl UsiEngine {
    pub fn new() -> UsiEngine {
        UsiEngine {
            position: Position::hirate(),
            searcher: Arc::new(Mutex::new(Searcher::new(DEFAULT_HASH_MB))),
            running: None,
            hash_mb: DEFAULT_HASH_MB,
        }
    }

    /// 標準入力から USI コマンドを読み続ける。`quit` で戻る。
    pub fn run(&mut self) {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            let Ok(line) = line else { break };
            if !self.handle(line.trim()) {
                break;
            }
        }
        self.abort_search();
    }

    /// 1 行処理する。`false` を返したら終了。
    fn handle(&mut self, line: &str) -> bool {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let Some(&command) = tokens.first() else {
            return true;
        };

        match command {
            "usi" => {
                respond(&format!("id name {ENGINE_NAME}"));
                respond(&format!("id author {ENGINE_AUTHOR}"));
                respond(&format!(
                    "option name USI_Hash type spin default {DEFAULT_HASH_MB} min 1 max 4096"
                ));
                respond("usiok");
            }
            "isready" => {
                // 置換表の確保をここで済ませておく（`go` を待たせない）。ロックを取れれば
                // 前の探索も終わっている、という確認も兼ねる。
                drop(self.searcher.lock());
                respond("readyok");
            }
            "setoption" => self.set_option(&tokens),
            "usinewgame" => {
                self.abort_search();
                if let Ok(mut searcher) = self.searcher.lock() {
                    searcher.clear();
                }
            }
            "position" => self.set_position(&tokens),
            "go" => self.go(&tokens[1..]),
            "stop" => self.request_stop(),
            "gameover" => self.abort_search(),
            "quit" => return false,
            // 対局中に盤を見たいときのための、プロトコル外の便利コマンド。
            "d" => print!("{}", self.position.to_ascii()),
            _ => {}
        }
        true
    }

    fn set_option(&mut self, tokens: &[&str]) {
        // `setoption name <名前> value <値>`
        let name = position_after(tokens, "name");
        let value = position_after(tokens, "value");
        if let (Some("USI_Hash"), Some(value)) = (name, value) {
            if let Ok(megabytes) = value.parse::<usize>() {
                self.hash_mb = megabytes.clamp(1, 4096);
                self.abort_search();
                self.searcher = Arc::new(Mutex::new(Searcher::new(self.hash_mb)));
            }
        }
    }

    fn set_position(&mut self, tokens: &[&str]) {
        self.abort_search();

        let moves_at = tokens.iter().position(|&t| t == "moves");
        let head: &[&str] = &tokens[1..moves_at.unwrap_or(tokens.len())];

        let sfen = match head.first() {
            Some(&"startpos") | None => HIRATE_SFEN.to_string(),
            Some(&"sfen") => head[1..].join(" "),
            // 素の SFEN を投げてくる相手にも付き合う。
            Some(_) => head.join(" "),
        };

        let moves: Vec<&str> = match moves_at {
            Some(index) => tokens[index + 1..].to_vec(),
            None => Vec::new(),
        };

        match Position::from_sfen_and_moves(&sfen, &moves) {
            Ok(position) => self.position = position,
            Err(error) => respond(&format!("info string 局面を組めない: {error}")),
        }
    }

    fn go(&mut self, tokens: &[&str]) {
        self.abort_search();

        let params = GoParams::parse(tokens);
        let limits = params.limits(self.position.side_to_move());
        let mut position = self.position.clone();
        let searcher = Arc::clone(&self.searcher);

        let stop = {
            let guard = searcher.lock().expect("探索器のロックが壊れた");
            let stop = guard.stop_flag();
            // **探索を始める前に旗を下ろす。** 下ろす場所を探索側に持つと、
            // `go` の直後に来た `stop` を取りこぼす。
            stop.store(false, Ordering::Relaxed);
            stop
        };

        let handle = std::thread::spawn(move || {
            let mut guard = searcher.lock().expect("探索器のロックが壊れた");
            let result = guard.search(&mut position, &limits, &mut |info| {
                respond(&format_info(info));
            });
            respond(&format_bestmove(&result));
        });

        self.running = Some(Running { handle, stop });
    }

    fn request_stop(&mut self) {
        if let Some(running) = &self.running {
            running.stop.store(true, Ordering::Relaxed);
        }
    }

    /// 走っている探索を止めて、終わるまで待つ。局面を差し替える前に必ず通す。
    fn abort_search(&mut self) {
        if let Some(running) = self.running.take() {
            running.stop.store(true, Ordering::Relaxed);
            let _ = running.handle.join();
        }
    }
}

impl Default for UsiEngine {
    fn default() -> UsiEngine {
        UsiEngine::new()
    }
}

impl Drop for UsiEngine {
    fn drop(&mut self) {
        self.abort_search();
    }
}

/// `setoption name X value Y` から名前や値を取り出す。
fn position_after<'a>(tokens: &[&'a str], keyword: &str) -> Option<&'a str> {
    tokens
        .iter()
        .position(|&t| t == keyword)
        .and_then(|index| tokens.get(index + 1))
        .copied()
}

fn format_info(info: &SearchInfo) -> String {
    let score = match info.mate_in() {
        Some(mate) => format!("score mate {mate}"),
        None => format!("score cp {}", info.score),
    };
    let pv = info
        .pv
        .iter()
        .map(|mv| mv.to_usi())
        .collect::<Vec<_>>()
        .join(" ");
    let mut line = format!(
        "info depth {} time {} nodes {} nps {} {score}",
        info.depth, info.elapsed_ms, info.nodes, info.nps
    );
    if !pv.is_empty() {
        line.push_str(" pv ");
        line.push_str(&pv);
    }
    line
}

fn format_bestmove(result: &SearchResult) -> String {
    match result.best_move {
        None => "bestmove resign".to_string(),
        Some(best) => match result.ponder_move {
            Some(ponder) => format!("bestmove {} ponder {}", best.to_usi(), ponder.to_usi()),
            None => format!("bestmove {}", best.to_usi()),
        },
    }
}

/// USI は行単位。**毎行 flush する** — バッファに溜めると GUI が応答無しと見なす。
fn respond(line: &str) {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;
    use houou_core::Move;

    #[test]
    fn go_parameters_are_read() {
        let params = GoParams::parse(&["btime", "300000", "wtime", "290000", "byoyomi", "3000"]);
        assert_eq!(params.btime, Some(300_000));
        assert_eq!(params.wtime, Some(290_000));
        assert_eq!(params.byoyomi, Some(3_000));
        assert!(!params.infinite);
    }

    #[test]
    fn infinite_means_no_time_budget() {
        let params = GoParams::parse(&["infinite"]);
        assert_eq!(params.budget_ms(Color::Black), None);
    }

    #[test]
    fn byoyomi_is_spent_every_move() {
        let params = GoParams::parse(&["btime", "0", "wtime", "0", "byoyomi", "5000"]);
        let budget = params.budget_ms(Color::Black).expect("予算がある");
        assert!(
            (4_000..5_000).contains(&budget),
            "秒読みをほぼ使い切るはず: {budget}"
        );
    }

    #[test]
    fn the_clock_is_read_for_the_side_to_move() {
        let params = GoParams::parse(&["btime", "600000", "wtime", "1000"]);
        let black = params.budget_ms(Color::Black).expect("予算がある");
        let white = params.budget_ms(Color::White).expect("予算がある");
        assert!(black > white, "手番側の残り時間を見ていない");
    }

    #[test]
    fn a_depth_limit_removes_the_time_budget() {
        let params = GoParams::parse(&["depth", "8"]);
        assert_eq!(params.depth, Some(8));
        assert_eq!(params.budget_ms(Color::Black), None);
    }

    #[test]
    fn position_startpos_with_moves_is_applied() {
        let mut engine = UsiEngine::new();
        engine.handle("position startpos moves 7g7f 3c3d");
        assert_eq!(engine.position.history_len(), 2);
        assert_eq!(engine.position.side_to_move(), Color::Black);
    }

    #[test]
    fn position_sfen_is_applied() {
        let mut engine = UsiEngine::new();
        engine.handle("position sfen 4k4/9/9/9/9/9/9/9/4K4 b - 1");
        assert_eq!(engine.position.occupied().count(), 2);
    }

    #[test]
    fn an_illegal_move_leaves_the_position_alone() {
        let mut engine = UsiEngine::new();
        let before = engine.position.to_sfen();
        engine.handle("position startpos moves 7g7e");
        assert_eq!(engine.position.to_sfen(), before, "非合法手を受け入れた");
    }

    #[test]
    fn quit_stops_the_loop() {
        let mut engine = UsiEngine::new();
        assert!(engine.handle("usi"));
        assert!(engine.handle("isready"));
        assert!(!engine.handle("quit"));
    }

    #[test]
    fn a_mated_position_resigns() {
        let result = SearchResult {
            best_move: None,
            ..Default::default()
        };
        assert_eq!(format_bestmove(&result), "bestmove resign");
    }

    #[test]
    fn a_mate_score_is_reported_as_mate_not_centipawns() {
        let info = SearchInfo {
            depth: 3,
            score: houou_core::eval::MATE - 3,
            nodes: 100,
            elapsed_ms: 1,
            nps: 100_000,
            pv: vec![Move::from_usi("G*2b").unwrap()],
        };
        let line = format_info(&info);
        assert!(line.contains("score mate 2"), "詰み手数が出ていない: {line}");
        assert!(line.ends_with("pv G*2b"));
    }
}
