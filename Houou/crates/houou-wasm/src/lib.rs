//! Houou の wasm 束ね。`Tsubame/examples/shogi-demo` の `ShogiEngine` プラグインが読む。
//!
//! # 走らせる場所は Worker
//!
//! この wasm は **Worker の中で回す前提**で、`search` は 1 手ぶんを最後まで読み切る
//! （途中で譲らない）。理由は 2 つ:
//!
//! - 反復深化は 1 段の途中で止めて再開する形になっていない。6ms ごとに譲るには
//!   探索を継続機械に畳み直す必要があり、割に合わない。
//! - `engine.ts` が既に「非同期エンジン（Worker / WASM）は `poll` で現在の状態だけを返す」
//!   と決めている。Worker はその契約にそのまま乗る。
//!
//! **シングルスレッド wasm なので `SharedArrayBuffer` も COOP/COEP も要らない。**
//! 既製の強エンジン（`-pthread` ビルド）が要求していた条件はここには当たらない。
//!
//! # 時計
//!
//! wasm32 に `std::time::Instant` は無いので、`Date.now()` を [`Searcher::set_clock`] へ渡す。
//! これを忘れると時間制限が効かず、`Instant` を呼んだ時点で panic する。

use houou_core::search::{SearchInfo, SearchLimits, Searcher};
use houou_core::{Move, Position};
use wasm_bindgen::prelude::*;

/// wasm 側の時計。`performance.now()` ではなく `Date.now()` を使うのは、
/// Worker でも window でも同じ 1 本で済むため。分解能 1ms で足りる。
fn now_ms() -> u64 {
    js_sys::Date::now() as u64
}

#[wasm_bindgen]
pub struct HououEngine {
    searcher: Searcher,
}

#[wasm_bindgen]
impl HououEngine {
    /// `hash_mb` は置換表の大きさ。ブラウザのメモリを考えて 16〜64 くらいが妥当。
    #[wasm_bindgen(constructor)]
    pub fn new(hash_mb: usize) -> HououEngine {
        console_error_panic_hook::set_once();
        let mut searcher = Searcher::new(hash_mb);
        searcher.set_clock(now_ms);
        HououEngine { searcher }
    }

    /// 対局をまたぐ状態（置換表・履歴）を捨てる。
    #[wasm_bindgen(js_name = newGame)]
    pub fn new_game(&mut self) {
        self.searcher.clear();
    }

    /// 1 手考える。**読み切るまで返らない**ので Worker の中で呼ぶこと。
    ///
    /// - `sfen` 開始局面、`moves` はそこからの USI 指し手を空白区切りで
    ///   （`EngineSearchRequest` の `sfen` / `moves` にそのまま対応する）
    /// - `on_info` は深さが 1 つ進むたびに JSON 文字列で呼ばれる。省略可。
    ///
    /// 戻り値は結果の JSON。合法手が無ければ `bestMove` が `null`。
    pub fn search(
        &mut self,
        sfen: &str,
        moves: &str,
        budget_ms: u32,
        max_depth: u32,
        on_info: Option<js_sys::Function>,
    ) -> Result<String, JsValue> {
        let move_list: Vec<&str> = moves.split_whitespace().collect();
        let mut position = Position::from_sfen_and_moves(sfen, &move_list)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;

        let limits = SearchLimits {
            depth: if max_depth == 0 {
                None
            } else {
                Some(max_depth)
            },
            nodes: None,
            time_ms: if budget_ms == 0 {
                None
            } else {
                Some(budget_ms as u64)
            },
        };

        let result = self.searcher.search(&mut position, &limits, &mut |info| {
            if let Some(callback) = &on_info {
                // 進捗の送信に失敗しても探索は続ける（受け手が居ないだけ）。
                let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(&info_json(info)));
            }
        });

        Ok(format!(
            "{{\"bestMove\":{},\"ponder\":{},\"scoreCp\":{},\"depth\":{},\"nodes\":{},\"elapsedMs\":{},\"mate\":{}}}",
            quote_move(result.best_move),
            quote_move(result.ponder_move),
            result.score,
            result.depth,
            result.nodes,
            result.elapsed_ms,
            mate_json(result.score),
        ))
    }

    /// 局面をそのまま評価する（探索しない）。デバッグと動作確認用。
    pub fn evaluate(&self, sfen: &str, moves: &str) -> Result<i32, JsValue> {
        let move_list: Vec<&str> = moves.split_whitespace().collect();
        let position = Position::from_sfen_and_moves(sfen, &move_list)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(houou_core::evaluate(&position))
    }

    /// 合法手を USI で列挙する。tsshogi との差分テストが使う入口。
    #[wasm_bindgen(js_name = legalMoves)]
    pub fn legal_moves(&self, sfen: &str, moves: &str) -> Result<Vec<String>, JsValue> {
        let move_list: Vec<&str> = moves.split_whitespace().collect();
        let mut position = Position::from_sfen_and_moves(sfen, &move_list)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        Ok(houou_core::generate_legal(&mut position)
            .iter()
            .map(|mv| mv.to_usi())
            .collect())
    }
}

/// エンジンの版。プラグインがログに出す。
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn info_json(info: &SearchInfo) -> String {
    let pv = info
        .pv
        .iter()
        .map(|mv| format!("\"{}\"", mv.to_usi()))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"depth\":{},\"scoreCp\":{},\"nodes\":{},\"nps\":{},\"elapsedMs\":{},\"mate\":{},\"pv\":[{pv}]}}",
        info.depth,
        info.score,
        info.nodes,
        info.nps,
        info.elapsed_ms,
        mate_json(info.score),
    )
}

fn quote_move(mv: Option<Move>) -> String {
    match mv {
        Some(mv) => format!("\"{}\"", mv.to_usi()),
        None => "null".to_string(),
    }
}

/// 詰みが見えていれば「あと何手で詰むか」。手番側が詰ますなら正。
fn mate_json(score: i32) -> String {
    use houou_core::eval::{MATE, MATE_THRESHOLD};
    if score >= MATE_THRESHOLD {
        ((MATE - score + 1) / 2).to_string()
    } else if score <= -MATE_THRESHOLD {
        (-((MATE + score + 1) / 2)).to_string()
    } else {
        "null".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // wasm 固有の部分（`Date.now` と `js_sys::Function`）はネイティブでは動かないので、
    // ここでは JSON の組み立てだけを見る。実際の動作は demo 側の e2e が押さえる。

    #[test]
    fn a_move_is_quoted_and_a_missing_move_is_null() {
        assert_eq!(quote_move(Move::from_usi("7g7f")), "\"7g7f\"");
        assert_eq!(quote_move(None), "null");
    }

    #[test]
    fn an_ordinary_score_has_no_mate_field() {
        assert_eq!(mate_json(120), "null");
        assert_eq!(mate_json(-120), "null");
    }

    #[test]
    fn a_mate_score_becomes_a_move_count() {
        use houou_core::eval::MATE;
        assert_eq!(mate_json(MATE - 1), "1", "1 手詰み");
        assert_eq!(mate_json(MATE - 3), "2", "3 手詰み = 自分の 2 手目で詰む");
        assert_eq!(mate_json(-(MATE - 3)), "-2", "詰まされる側は負");
    }

    #[test]
    fn info_json_is_well_formed() {
        let info = SearchInfo {
            depth: 5,
            score: 42,
            nodes: 1234,
            elapsed_ms: 10,
            nps: 123_400,
            pv: vec![
                Move::from_usi("7g7f").unwrap(),
                Move::from_usi("3c3d").unwrap(),
            ],
        };
        let json = info_json(&info);
        assert!(json.contains("\"depth\":5"));
        assert!(json.contains("\"scoreCp\":42"));
        assert!(json.contains("\"mate\":null"));
        assert!(json.ends_with("\"pv\":[\"7g7f\",\"3c3d\"]}"));
    }
}
