//! 適応的レンダースケール劣化（ADR-0129）。
//!
//! **静的 DPR cap は持たない**。平常時はフル DPR で描画し（画質を犠牲にしない）、フレーム時間が予算を
//! 継続的に超える、またはサーマル/電力シグナルが逼迫を示すときだけ `content_scale` を段階的に下げて
//! バッファを縮小し、CSS/サーフェス側で拡大表示する。逼迫が解消したら**ヒステリシス付きで**元の
//! スケールへ復帰する（境界での振動を防ぐ）。
//!
//! 判定とスケール駆動は Platform Front/バックエンド（フレーム時間・熱シグナルを持つ層）が行う。本
//! モジュールはその純粋な状態機械で、core のレイヤ判定・layout（CSS px）は不変。スケール変更時は
//! ヒットテスト座標も同じ scale で再スケールする（[`hit_test_logical`]）。`viewport_metrics` と同じく
//! プラットフォーム非依存の共有導出で、platform が駆動する。

/// 名前付き tunable（ADR-0129・プレースホルダ値可）。マジックナンバーを状態機械へ散らさない正本。
pub mod tunables {
    /// フレーム時間予算（60Hz・ms）。これを継続的に超えると劣化を検討する。
    pub const FRAME_BUDGET_60HZ_MS: f64 = 1000.0 / 60.0;
    /// フレーム時間予算（120Hz・ms）。
    pub const FRAME_BUDGET_120HZ_MS: f64 = 1000.0 / 120.0;
    /// 逼迫と判定し 1 段下げるまでの連続超過フレーム数（単発スパイクで落とさない）。
    pub const PRESSURE_FRAMES_TO_DEGRADE: u32 = 3;
    /// 1 段戻すまでの連続余裕フレーム数。劣化（速い）より復帰（遅い）を長くすることがヒステリシスで、
    /// 境界での振動を防ぐ（ADR-0129）。
    pub const RECOVERY_FRAMES_TO_RESTORE: u32 = 30;
    /// レンダースケール段階（フル→劣化）。`1.0` が平常時のフル DPR。base DPR への乗数で、下げると
    /// バッファが縮む。モバイル DPR(≈2.5–3) では最小 0.5 でも実効スケールは 1.0 を上回る。
    pub const SCALE_STEPS: [f32; 4] = [1.0, 0.85, 0.7, 0.5];
    /// これを超える `dt`（ms）は実フレーム時間ではなく on-demand ループ（ADR-0126）のアイドル
    /// ギャップとみなし、「計測不能」として over/under どちらのストリークにも数えない（#667・
    /// プレースホルダ値）。web の frame ループはアイドルへ落ちると rAF を止めるため、アイドル明けの
    /// 次フレームは前フレームからの経過時間（アイドル時間そのもの）がそのまま dt になり得る。
    pub const MAX_PLAUSIBLE_FRAME_MS: f64 = 250.0;
}

/// 適応的レンダースケールの状態機械（ADR-0129）。フレーム時間/熱の逼迫で段階的に劣化し、余裕が続けば
/// ヒステリシス付きで 1 段ずつ復帰する。
#[derive(Debug, Clone)]
pub struct RenderScaleGovernor {
    step_index: usize,
    over_budget_streak: u32,
    under_budget_streak: u32,
    budget_ms: f64,
}

impl RenderScaleGovernor {
    /// リフレッシュ予算（ms。例 [`tunables::FRAME_BUDGET_60HZ_MS`]）を与えてフル DPR で開始する。
    pub fn new(budget_ms: f64) -> Self {
        Self {
            step_index: 0,
            over_budget_streak: 0,
            under_budget_streak: 0,
            budget_ms,
        }
    }

    /// 現在のレンダースケール（base DPR への乗数）。平常時は `1.0`（フル DPR）。
    pub fn scale(&self) -> f32 {
        tunables::SCALE_STEPS[self.step_index]
    }

    /// 平常時（フル DPR・劣化なし）か。
    pub fn is_full_scale(&self) -> bool {
        self.step_index == 0
    }

    /// 1 フレームの計測を反映する。`frame_ms` が予算超過、または `thermal_pressure` が真なら逼迫
    /// フレーム。逼迫が `PRESSURE_FRAMES_TO_DEGRADE` 連続したら 1 段劣化、余裕が
    /// `RECOVERY_FRAMES_TO_RESTORE` 連続したら 1 段復帰する。スケールが変わったら `true` を返す。
    pub fn note_frame(&mut self, frame_ms: f64, thermal_pressure: bool) -> bool {
        let over = thermal_pressure || frame_ms > self.budget_ms;
        if over {
            self.over_budget_streak += 1;
            self.under_budget_streak = 0;
        } else {
            self.under_budget_streak += 1;
            self.over_budget_streak = 0;
        }

        // 劣化（速い）：逼迫が連続し、まだ最下段でなければ 1 段下げる。
        if self.over_budget_streak >= tunables::PRESSURE_FRAMES_TO_DEGRADE
            && self.step_index + 1 < tunables::SCALE_STEPS.len()
        {
            self.step_index += 1;
            self.over_budget_streak = 0;
            return true;
        }

        // 復帰（遅い＝ヒステリシス）：余裕が長く連続し、フル DPR でなければ 1 段戻す。
        if self.under_budget_streak >= tunables::RECOVERY_FRAMES_TO_RESTORE && self.step_index > 0 {
            self.step_index -= 1;
            self.under_budget_streak = 0;
            return true;
        }

        false
    }
}

/// rAF timestamp 列から [`RenderScaleGovernor`] を駆動するフレームループドライバ（ADR-0129）。
///
/// バックエンド（web の frame ループ等）は生の frame 時間 dt を持たず rAF timestamp しか持たないため、
/// このドライバが**連続 timestamp の差分から dt を導出**して governor に渡す。最初のフレームは前フレーム
/// timestamp が無いので dt を測れず、劣化判定に寄与させない（スパイクとして誤検出しない）。スケールが
/// 変わったフレームだけ新しい render_scale を返し、バックエンドはそのときだけ buffer resize を起こす。
///
/// governor と同じく layout（CSS px）・ヒットテスト（論理座標）は不変で、render_scale は buffer 寸法と
/// `effective_content_scale` にのみ効く（CSS サイズは不変でブラウザが拡大表示する）。
#[derive(Debug, Clone)]
pub struct RenderScaleDriver {
    governor: RenderScaleGovernor,
    last_timestamp_ms: Option<f64>,
}

impl RenderScaleDriver {
    /// リフレッシュ予算（ms。例 [`tunables::FRAME_BUDGET_60HZ_MS`]）を与えてフル DPR で開始する。
    pub fn new(budget_ms: f64) -> Self {
        Self {
            governor: RenderScaleGovernor::new(budget_ms),
            last_timestamp_ms: None,
        }
    }

    /// 1 フレームの rAF `timestamp_ms`（単調増加）を反映する。前フレームとの差分を frame 時間として
    /// governor に渡す。スケールが変わったら新しい render_scale（base DPR への乗数）を `Some` で返す。
    /// 最初のフレーム（基準 timestamp が無い）は dt を測れないので必ず `None`。`dt` が
    /// [`tunables::MAX_PLAUSIBLE_FRAME_MS`] を超えるときも同様に `None`（#667）：on-demand ループ
    /// （ADR-0126）のアイドル明けはアイドル時間そのものが dt になり得るため、実フレーム時間として
    /// governor に渡さず「計測不能」として無視する（over/under どちらのストリークも変化しない）。
    pub fn note_frame(&mut self, timestamp_ms: f64, thermal_pressure: bool) -> Option<f32> {
        let changed = match self.last_timestamp_ms {
            Some(prev) => {
                let dt = timestamp_ms - prev;
                if dt > tunables::MAX_PLAUSIBLE_FRAME_MS {
                    false
                } else {
                    self.governor.note_frame(dt, thermal_pressure)
                }
            }
            None => false,
        };
        self.last_timestamp_ms = Some(timestamp_ms);
        if changed {
            Some(self.governor.scale())
        } else {
            None
        }
    }

    /// 現在の render_scale（base DPR への乗数）。平常時は `1.0`。
    pub fn scale(&self) -> f32 {
        self.governor.scale()
    }
}

/// base DPR とレンダースケール乗数から実効 content scale を求める。バッファ寸法導出
/// （`ViewportMetrics`）とヒットテストはこの同じ値を使う（ADR-0129）。
pub fn effective_content_scale(base_dpr: f32, render_scale: f32) -> f32 {
    base_dpr * render_scale
}

/// 物理（バッファ）座標を実効 content scale で論理（CSS px）へ写す。スケール変更時もバッファ寸法と
/// ヒットテストが同じ scale を使うことで、ポインタ座標が描画と整合する（ADR-0129）。
pub fn hit_test_logical(physical: (f32, f32), effective_scale: f32) -> (f32, f32) {
    (physical.0 / effective_scale, physical.1 / effective_scale)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn governor() -> RenderScaleGovernor {
        RenderScaleGovernor::new(tunables::FRAME_BUDGET_60HZ_MS)
    }

    /// `frame_ms` のフレームを `n` 回流す。
    fn feed(g: &mut RenderScaleGovernor, frame_ms: f64, thermal: bool, n: u32) {
        for _ in 0..n {
            g.note_frame(frame_ms, thermal);
        }
    }

    #[test]
    fn starts_at_full_dpr_with_no_static_cap() {
        let g = governor();
        assert_eq!(g.scale(), 1.0, "平常時はフル DPR（静的 cap なし）");
        assert!(g.is_full_scale());
    }

    #[test]
    fn a_single_slow_frame_does_not_degrade() {
        // 単発スパイクでは落とさない（PRESSURE_FRAMES_TO_DEGRADE 未満）。
        let mut g = governor();
        let changed = g.note_frame(50.0, false);
        assert!(!changed);
        assert_eq!(g.scale(), 1.0);
    }

    #[test]
    fn sustained_over_budget_degrades_stepwise() {
        let mut g = governor();
        // 逼迫が PRESSURE 連続 → 1 段劣化。
        feed(&mut g, 50.0, false, tunables::PRESSURE_FRAMES_TO_DEGRADE);
        assert_eq!(g.scale(), tunables::SCALE_STEPS[1]);
        // さらに連続 → もう 1 段。
        feed(&mut g, 50.0, false, tunables::PRESSURE_FRAMES_TO_DEGRADE);
        assert_eq!(g.scale(), tunables::SCALE_STEPS[2]);
    }

    #[test]
    fn degradation_stops_at_minimum_step() {
        let mut g = governor();
        // 大量に逼迫しても最下段でクランプ。
        feed(&mut g, 100.0, false, tunables::PRESSURE_FRAMES_TO_DEGRADE * 10);
        assert_eq!(g.scale(), *tunables::SCALE_STEPS.last().unwrap());
    }

    #[test]
    fn thermal_pressure_degrades_even_within_frame_budget() {
        let mut g = governor();
        // フレーム時間は予算内でも、熱逼迫シグナルが続けば劣化する。
        feed(&mut g, 1.0, true, tunables::PRESSURE_FRAMES_TO_DEGRADE);
        assert_eq!(g.scale(), tunables::SCALE_STEPS[1]);
    }

    #[test]
    fn recovery_requires_sustained_headroom_then_steps_back_one_at_a_time() {
        let mut g = governor();
        feed(&mut g, 50.0, false, tunables::PRESSURE_FRAMES_TO_DEGRADE); // step1
        assert_eq!(g.scale(), tunables::SCALE_STEPS[1]);

        // RECOVERY 未満の余裕では復帰しない（ヒステリシス）。
        feed(&mut g, 1.0, false, tunables::RECOVERY_FRAMES_TO_RESTORE - 1);
        assert_eq!(g.scale(), tunables::SCALE_STEPS[1]);

        // ちょうど RECOVERY 連続で 1 段だけ復帰。
        g.note_frame(1.0, false);
        assert_eq!(g.scale(), tunables::SCALE_STEPS[0]);
        assert!(g.is_full_scale());
    }

    #[test]
    fn alternating_pressure_does_not_oscillate() {
        let mut g = governor();
        feed(&mut g, 50.0, false, tunables::PRESSURE_FRAMES_TO_DEGRADE); // step1
        assert_eq!(g.scale(), tunables::SCALE_STEPS[1]);

        // 境界で over/under を交互に与える：under streak が毎回リセットされ復帰しない。over streak も
        // PRESSURE に届かず追加劣化もしない＝振動せず step1 のまま安定。
        for _ in 0..100 {
            g.note_frame(1.0, false); // under（streak=1）
            g.note_frame(50.0, false); // over（under streak リセット）
        }
        assert_eq!(g.scale(), tunables::SCALE_STEPS[1], "境界で振動しない");
    }

    #[test]
    fn named_tunables_express_hysteresis_and_steps() {
        // 復帰は劣化より長い連続を要する＝ヒステリシス。段階は単調減少（フル→劣化）。
        assert!(tunables::RECOVERY_FRAMES_TO_RESTORE > tunables::PRESSURE_FRAMES_TO_DEGRADE);
        assert_eq!(tunables::SCALE_STEPS[0], 1.0);
        for pair in tunables::SCALE_STEPS.windows(2) {
            assert!(pair[1] < pair[0], "スケール段階は単調減少");
        }
    }

    /// budget を継続的に超える dt を刻む timestamp 列（0, step, 2*step, ...）。
    fn timestamps(step: f64, n: u32) -> Vec<f64> {
        (0..n).map(|i| i as f64 * step).collect()
    }

    #[test]
    fn driver_first_frame_never_degrades_lacking_a_dt_baseline() {
        // 最初の 1 フレームは前 timestamp が無く dt を測れない。単発の劣化には数えない。
        let mut d = RenderScaleDriver::new(tunables::FRAME_BUDGET_60HZ_MS);
        assert_eq!(d.note_frame(0.0, false), None);
        assert_eq!(d.scale(), 1.0);
    }

    #[test]
    fn driver_derives_frame_time_from_timestamp_deltas_and_degrades() {
        // 50ms 間隔（>16.6ms 予算）の timestamp を刻むと、PRESSURE 連続で 1 段劣化を返す。
        let mut d = RenderScaleDriver::new(tunables::FRAME_BUDGET_60HZ_MS);
        let ts = timestamps(50.0, tunables::PRESSURE_FRAMES_TO_DEGRADE + 1);
        let mut changes: Vec<f32> = Vec::new();
        for &t in &ts {
            if let Some(scale) = d.note_frame(t, false) {
                changes.push(scale);
            }
        }
        // 基準フレーム 1 つ＋PRESSURE 連続の超過で、ちょうど 1 回だけスケール変更を返す。
        assert_eq!(changes, vec![tunables::SCALE_STEPS[1]]);
        assert_eq!(d.scale(), tunables::SCALE_STEPS[1]);
    }

    #[test]
    fn driver_ignores_idle_gaps_larger_than_the_plausible_frame_ceiling() {
        // web の on-demand フレームループ（ADR-0126）は pending な視覚更新が無ければ rAF を止めて
        // 完全にアイドルへ落ちる。アイドル明けの次フレームは、前フレームからの経過時間（＝アイドル
        // 時間そのもの、数百 ms 以上）がそのまま dt になる。実際には負荷が全く無いのに、これを
        // 「予算超過」と誤検出して劣化してはならない（#667）。
        let mut d = RenderScaleDriver::new(tunables::FRAME_BUDGET_60HZ_MS);
        assert_eq!(d.note_frame(0.0, false), None);

        // 大きな間隔（アイドル明け）を挟んだ単発フレームを PRESSURE_FRAMES_TO_DEGRADE を超えて
        // 繰り返しても、一度も劣化しない（間欠的な on-demand ウェイクを模したケース）。
        let mut t = 0.0;
        for _ in 0..20 {
            t += tunables::MAX_PLAUSIBLE_FRAME_MS + 500.0;
            assert_eq!(d.note_frame(t, false), None);
        }
        assert_eq!(d.scale(), 1.0, "アイドル明けの単発フレームでは劣化しない");
    }

    #[test]
    fn driver_returns_none_on_frames_without_a_scale_change() {
        // 予算内（8ms 間隔）の timestamp では一度もスケールが変わらない＝毎フレーム None。
        let mut d = RenderScaleDriver::new(tunables::FRAME_BUDGET_60HZ_MS);
        for &t in &timestamps(8.0, 20) {
            assert_eq!(d.note_frame(t, false), None);
        }
        assert_eq!(d.scale(), 1.0);
    }

    #[test]
    fn driver_effective_scale_shrinks_buffer_but_not_the_logical_viewport() {
        // 劣化しても実効 content scale（= buffer 寸法の乗数）だけが縮み、CSS/論理ビューポートは不変。
        // ヒットテストは論理座標のままなので、劣化前後で論理←→物理往復が整合する（ADR-0129）。
        let base_dpr = 3.0_f32;
        let mut d = RenderScaleDriver::new(tunables::FRAME_BUDGET_60HZ_MS);
        for &t in &timestamps(50.0, tunables::PRESSURE_FRAMES_TO_DEGRADE + 1) {
            d.note_frame(t, false);
        }
        let render_scale = d.scale();
        assert!(render_scale < 1.0, "逼迫で劣化している");
        let eff = effective_content_scale(base_dpr, render_scale);
        assert!(eff < base_dpr, "buffer 乗数は base DPR より小さい");
        // 論理座標のヒットテストは effective_scale で物理に写しても元へ戻る（描画と整合）。
        let logical = (120.0_f32, 80.0_f32);
        let physical = (logical.0 * eff, logical.1 * eff);
        let back = hit_test_logical(physical, eff);
        assert!((back.0 - logical.0).abs() < 1e-3);
        assert!((back.1 - logical.1).abs() < 1e-3);
    }

    #[test]
    fn hit_test_stays_consistent_across_scale_changes() {
        // どのスケール段でも、論理→物理→論理 の往復が同じ scale を使えば一致する（ADR-0129）。
        let base_dpr = 3.0_f32; // モバイル相当
        let logical = (120.0_f32, 80.0_f32);
        for &render_scale in &tunables::SCALE_STEPS {
            let eff = effective_content_scale(base_dpr, render_scale);
            assert!(eff >= 1.0, "モバイル DPR では実効スケールが 1.0 を下回らない");
            let physical = (logical.0 * eff, logical.1 * eff);
            let back = hit_test_logical(physical, eff);
            assert!((back.0 - logical.0).abs() < 1e-3);
            assert!((back.1 - logical.1).abs() < 1e-3);
        }
    }
}
