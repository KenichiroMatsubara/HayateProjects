//! Houou のネイティブ入口。
//!
//! `usi` が本番の入口で、それ以外は生成器と探索を測る道具。
//! Phase 4 で教師局面生成（`gensfen`）が同じ入口に増える。
//!
//! 引数の解析を手書きしているのは `houou-core` の依存ゼロ方針に合わせるため。
//! ここだけ clap を引いても良いが、サブコマンドがこの数のうちは割に合わない。

mod usi;

use houou_core::search::{SearchLimits, Searcher};
use houou_core::{perft, perft_divide, Position, HIRATE_SFEN};
use std::time::Instant;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("usi");

    let code = match command {
        "usi" => {
            usi::UsiEngine::new().run();
            0
        }
        "perft" => cmd_perft(&args[1..], false),
        "divide" => cmd_perft(&args[1..], true),
        "bench" => cmd_bench(),
        "search" => cmd_search(&args[1..]),
        "sfen" => cmd_sfen(&args[1..]),
        _ => {
            print_usage();
            0
        }
    };
    std::process::exit(code);
}

fn print_usage() {
    eprintln!(
        "houou — 将棋エンジン

使い方:
  houou usi                     USI プロトコルで喋る（将棋所 / ShogiHome の接続先）
  houou search <深さ> [SFEN]    1 局面を考えて読み筋を出す
  houou perft <深さ> [SFEN]     深さまでの合法手の総数を数える
  houou divide <深さ> [SFEN]    初手ごとの内訳を出す（食い違いの追跡用）
  houou bench                   平手の perft と探索で nps を測る
  houou sfen [SFEN]             局面を盤の形で表示して SFEN を書き戻す

引数を省くと usi。SFEN を省くと平手。速度は必ず release ビルドで測ること。"
    );
}

/// 位置引数から深さと SFEN を取り出す。SFEN は空白を含むので残り全部を繋ぐ。
fn parse_depth_and_sfen(args: &[String]) -> Result<(u32, String), String> {
    let depth: u32 = args
        .first()
        .ok_or_else(|| "深さが要る".to_string())?
        .parse()
        .map_err(|_| format!("深さが数でない: {}", args[0]))?;
    let sfen = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        HIRATE_SFEN.to_string()
    };
    Ok((depth, sfen))
}

fn cmd_perft(args: &[String], divide: bool) -> i32 {
    let (depth, sfen) = match parse_depth_and_sfen(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    let mut position = match Position::from_sfen(&sfen) {
        Ok(position) => position,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    let start = Instant::now();
    let total = if divide {
        let breakdown = perft_divide(&mut position, depth);
        for (mv, nodes) in &breakdown {
            println!("{}: {}", mv.to_usi(), nodes);
        }
        breakdown.iter().map(|(_, nodes)| nodes).sum()
    } else {
        perft(&mut position, depth)
    };
    let elapsed = start.elapsed();

    println!("nodes {total}");
    println!("time  {:.3}s", elapsed.as_secs_f64());
    println!("nps   {}", nps(total, elapsed.as_secs_f64()));
    0
}

fn cmd_search(args: &[String]) -> i32 {
    let (depth, sfen) = match parse_depth_and_sfen(args) {
        Ok(parsed) => parsed,
        Err(message) => {
            eprintln!("{message}");
            return 2;
        }
    };
    let mut position = match Position::from_sfen(&sfen) {
        Ok(position) => position,
        Err(error) => {
            eprintln!("{error}");
            return 2;
        }
    };

    print!("{}", position.to_ascii());
    let mut searcher = Searcher::new(128);
    let result = searcher.search(&mut position, &SearchLimits::depth(depth), &mut |info| {
        let score = match info.mate_in() {
            Some(mate) => format!("mate {mate}"),
            None => format!("{:+}", info.score),
        };
        let pv = info
            .pv
            .iter()
            .map(|mv| mv.to_usi())
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "depth {:>2}  {:>8}  {:>12} nodes  {:>10} nps  {pv}",
            info.depth, score, info.nodes, info.nps
        );
    });

    match result.best_move {
        Some(mv) => println!("\nbestmove {}", mv.to_usi()),
        None => println!("\nbestmove resign（詰み）"),
    }
    0
}

fn cmd_bench() -> i32 {
    let code = bench_perft();
    if code != 0 {
        return code;
    }
    bench_search();
    0
}

/// 探索の nps。**perft の nps とは別物** — 評価と枝刈りが乗るぶん必ず落ちる。
/// 強さに直結するのはこちらの値。
fn bench_search() {
    println!("\n平手からの探索。評価と枝刈りを含む実効 nps。\n");
    println!("{:>5}  {:>14}  {:>9}  {:>12}  pv", "depth", "nodes", "time", "nps");

    let mut position = Position::hirate();
    let mut searcher = Searcher::new(128);
    searcher.search(&mut position, &SearchLimits::depth(10), &mut |info| {
        let pv = info
            .pv
            .iter()
            .take(6)
            .map(|mv| mv.to_usi())
            .collect::<Vec<_>>()
            .join(" ");
        println!(
            "{:>5}  {:>14}  {:>8.3}s  {:>12}  {pv}",
            info.depth,
            info.nodes,
            info.elapsed_ms as f64 / 1000.0,
            info.nps
        );
    });
}

fn bench_perft() -> i32 {
    println!("平手 perft。既知の値と突き合わせながら nps を測る。\n");
    println!("{:>5}  {:>14}  {:>9}  {:>12}", "depth", "nodes", "time", "nps");

    for (index, &expected) in houou_core::perft::HIRATE_PERFT.iter().take(5).enumerate() {
        let depth = index as u32 + 1;
        let mut position = Position::hirate();
        let start = Instant::now();
        let nodes = perft(&mut position, depth);
        let seconds = start.elapsed().as_secs_f64();

        let verdict = if nodes == expected {
            "ok".to_string()
        } else {
            format!("食い違い (期待 {expected})")
        };
        println!(
            "{depth:>5}  {nodes:>14}  {:>8.3}s  {:>12}  {verdict}",
            seconds,
            nps(nodes, seconds)
        );
        if nodes != expected {
            eprintln!("\nperft が合っていない。生成器を直すこと。");
            return 1;
        }
    }
    0
}

fn cmd_sfen(args: &[String]) -> i32 {
    let sfen = if args.is_empty() {
        HIRATE_SFEN.to_string()
    } else {
        args.join(" ")
    };
    match Position::from_sfen(&sfen) {
        Ok(position) => {
            print!("{}", position.to_ascii());
            println!("sfen {}", position.to_sfen());
            0
        }
        Err(error) => {
            eprintln!("{error}");
            2
        }
    }
}

fn nps(nodes: u64, seconds: f64) -> u64 {
    if seconds <= 0.0 {
        0
    } else {
        (nodes as f64 / seconds) as u64
    }
}
