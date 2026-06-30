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
