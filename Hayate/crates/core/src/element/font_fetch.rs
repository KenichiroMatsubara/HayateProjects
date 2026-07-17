//! オンデマンドのフォント取得の管理。
//!
//! `.notdef` 検出（ADR-0042）が `FetchFont { family }` イベントを発行し、プラットフォーム
//! アダプタが face を取得して成功時に `register_font` を、失敗時に失敗を報告する。この
//! トラッカーは family ごとに今 `FetchFont` を発行すべきかを判断する。狙いは次の通り:
//!
//! - 取得が処理中の間は重複イベントを抑止する、
//! - 失敗した取得が family を永久にラッチしない。後続フレームで再要求できるようになる、
//! - 失敗し続ける family は有限の予算で見切り、ログや再要求が暴走しないようにする。

use std::collections::{HashMap, HashSet};

/// core が見切るまでの 1 family あたりの最大取得試行回数。アダプタが試行を
/// 間引く（バックオフ）一方、core は回数を上限で抑え、到達不能な family が
/// 永久に再要求されないようにする。
pub(crate) const MAX_FETCH_ATTEMPTS: u32 = 3;

/// 取得失敗の報告を受けて core が下した判断。
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FailureOutcome {
    /// 予算が残っている。family は後続フレームで再要求される。
    WillRetry,
    /// 予算を使い切った。family は見切られ、再要求されない。
    GaveUp,
}

/// オンデマンドフォント読み込みの family ごとの取得状態。
#[derive(Default)]
pub(crate) struct FontFetchTracker {
    /// `FetchFont` で要求済みで結果待ちの family。取得が未完の間、フレームをまたいだ
    /// 重複イベントを抑止する。
    in_flight: HashSet<String>,
    /// family ごとの失敗試行回数。読み込み成功または見切りまで保持する。
    attempts: HashMap<String, u32>,
    /// リトライ予算を使い切った family。二度と要求せず、呼び出し側が高々一度ログする。
    exhausted: HashSet<String>,
}

impl FontFetchTracker {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 今 `family` に対して `FetchFont` を発行すべきか。処理中でも見切り済みでもない場合のみ。
    pub(crate) fn should_request(&self, family: &str) -> bool {
        !self.in_flight.contains(family) && !self.exhausted.contains(family)
    }

    /// `family` に対し `FetchFont` を発行したことを記録する。
    pub(crate) fn mark_requested(&mut self, family: &str) {
        self.in_flight.insert(family.to_string());
    }

    /// `family` の読み込み成功を記録する。全状態をクリアし、同じ family に対する
    /// 後続の `.notdef` で改めて要求できるようにする。
    pub(crate) fn mark_loaded(&mut self, family: &str) {
        self.in_flight.remove(family);
        self.attempts.remove(family);
        self.exhausted.remove(family);
    }

    /// `family` の取得失敗を記録する。リトライされるか（`WillRetry`）見切られたか
    /// （`GaveUp`）を返す。
    pub(crate) fn mark_failed(&mut self, family: &str) -> FailureOutcome {
        self.in_flight.remove(family);
        let count = self.attempts.entry(family.to_string()).or_insert(0);
        *count += 1;
        if *count >= MAX_FETCH_ATTEMPTS {
            self.exhausted.insert(family.to_string());
            self.attempts.remove(family);
            FailureOutcome::GaveUp
        } else {
            FailureOutcome::WillRetry
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_until_in_flight_then_clears_on_load() {
        let mut t = FontFetchTracker::new();
        assert!(t.should_request("Noto Sans JP"));
        t.mark_requested("Noto Sans JP");
        assert!(
            !t.should_request("Noto Sans JP"),
            "in-flight suppresses re-request"
        );
        t.mark_loaded("Noto Sans JP");
        assert!(
            t.should_request("Noto Sans JP"),
            "a loaded family can be requested afresh"
        );
    }

    #[test]
    fn failure_reopens_until_budget_is_exhausted() {
        let mut t = FontFetchTracker::new();
        for _ in 0..MAX_FETCH_ATTEMPTS - 1 {
            t.mark_requested("Noto Sans JP");
            assert_eq!(t.mark_failed("Noto Sans JP"), FailureOutcome::WillRetry);
            assert!(
                t.should_request("Noto Sans JP"),
                "a retryable failure reopens the request"
            );
        }
        t.mark_requested("Noto Sans JP");
        assert_eq!(t.mark_failed("Noto Sans JP"), FailureOutcome::GaveUp);
        assert!(
            !t.should_request("Noto Sans JP"),
            "an exhausted family is never requested again"
        );
    }
}
