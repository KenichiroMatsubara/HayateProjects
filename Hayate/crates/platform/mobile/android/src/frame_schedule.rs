//! On-demand フレームループの起床/継続判定（ADR-0117 / ADR-0126）。host-testable。
//!
//! Android のメインループは従来 ~16ms ごとに**無条件**で `pump_frame`＋render を回し、
//! idle でも 60–120Hz で GPU を焼いていた（ADR-0126 が指摘した契約違反）。本モジュールは
//! 「このイテレーションでフレームを出すべきか」を純粋に判定する coalescer で、android 依存を
//! 持たないためホストの `cargo test` で振る舞いを固定できる。実ループ（`app_tsubame`）は
//! これを使って wake（入力到着・lifecycle・reload・signal 変化）と継続（進行中 transition /
//! カーソル点滅 / スクロール物理 = `visual_dirty`）のあるときだけ pump+present する。

/// on-demand フレームループの wake/継続を集約する純粋判定器。
///
/// 2 つの状態だけを持つ:
/// - `wake_requested`: 外部 wake 源（入力到着・lifecycle・reload・非同期 signal 変化・初回）が
///   立てるフラグ。`note_frame_rendered` で消費される。
/// - `continuation_pending`: 直近の描画後に残る pending visual work。次フレームを自走させる。
///
/// idle（どちらも false）では [`wants_frame`](Self::wants_frame) が false を返し、1 枚も出さない。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OnDemandFrameLoop {
    wake_requested: bool,
    continuation_pending: bool,
}

impl OnDemandFrameLoop {
    /// 冷間始動を要求した状態で構築する（起動直後の最初の 1 フレームは必ず出す）。
    pub fn started() -> Self {
        OnDemandFrameLoop {
            wake_requested: true,
            continuation_pending: false,
        }
    }

    /// wake 源（入力到着・lifecycle・reload・非同期 signal 変化）を 1 つ記録する。冪等で、
    /// 同一イテレーション内の複数 wake は 1 フレームに集約される。
    pub fn request_wake(&mut self) {
        self.wake_requested = true;
    }

    /// このイテレーションでフレームを produce すべきか。wake 要求か継続 pending のいずれかが
    /// あれば true。idle では false＝フレームを 1 枚も出さない。
    pub fn wants_frame(&self) -> bool {
        self.wake_requested || self.continuation_pending
    }

    /// フレームを 1 枚 produce した直後に呼ぶ。wake 要求を消費し、描画後に残る pending visual
    /// work（進行中 transition / カーソル点滅 / スクロール物理）を継続として記録する。
    pub fn note_frame_rendered(&mut self, pending_visual_work: bool) {
        self.wake_requested = false;
        self.continuation_pending = pending_visual_work;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn started_loop_wants_the_first_frame() {
        // 冷間始動：起動直後は wake 源が無くても最初の 1 フレームを出す。
        let loop_ = OnDemandFrameLoop::started();
        assert!(loop_.wants_frame());
    }

    #[test]
    fn goes_idle_after_a_frame_with_no_pending_visual_work() {
        // 描画後に pending visual work が無ければ idle：次イテレーションはフレームを出さない。
        let mut loop_ = OnDemandFrameLoop::started();
        loop_.note_frame_rendered(false);
        assert!(!loop_.wants_frame(), "idle ではフレームを 1 枚も出さない");
    }

    #[test]
    fn keeps_running_while_visual_work_is_pending_then_stops() {
        // 進行中 transition / カーソル点滅 / スクロール物理がある間は継続フレームを自走させ、
        // 解消したフレームの後に idle へ落ちる（退行なし）。
        let mut loop_ = OnDemandFrameLoop::started();
        loop_.note_frame_rendered(true);
        assert!(loop_.wants_frame());
        loop_.note_frame_rendered(true);
        assert!(loop_.wants_frame());
        loop_.note_frame_rendered(false);
        assert!(!loop_.wants_frame());
    }

    #[test]
    fn wake_cold_starts_an_idle_loop() {
        // idle から入力到着・非同期 signal 変化で冷間始動する。
        let mut loop_ = OnDemandFrameLoop::started();
        loop_.note_frame_rendered(false);
        assert!(!loop_.wants_frame());

        loop_.request_wake();
        assert!(loop_.wants_frame(), "wake は idle ループを冷間始動する");
    }

    #[test]
    fn multiple_wakes_coalesce_into_one_frame() {
        // 同一イテレーション内の複数 wake は 1 フレームに集約され、描画でまとめて消費される。
        let mut loop_ = OnDemandFrameLoop::started();
        loop_.note_frame_rendered(false);
        loop_.request_wake();
        loop_.request_wake();
        loop_.request_wake();
        assert!(loop_.wants_frame());

        // 1 フレーム描画（pending なし）で wake をすべて消費 → idle へ。
        loop_.note_frame_rendered(false);
        assert!(!loop_.wants_frame());
    }
}
