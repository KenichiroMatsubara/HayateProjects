//! 実効ビジュアルのトランジション補間（ADR-0089 / ADR-0093）。
//!
//! 解決済みの実効ビジュアル（ADR-0067）が連続プロパティを変更し、変更後の
//! `transition-duration` が正なら、レンダリング層はそのプロパティを画面上の値
//! （`from`）から新たに解決した目標値へ、`transition-timing` のイージングで補間する。
//! トリガは `resolve_effective` 接合部でのプロパティ単位の差分なので、擬似クラスの
//! 切り替え・`setStyle`・継承の変化を区別なく扱う（Blink の computed-style 差分と同じ）。
//! enum 値や離散プロパティは補間せず、即座に目標値を取る。状態は要素×プロパティ単位で
//! 保持し、各プロパティが独立した `from` と開始時刻から補間する。補間は
//! `render(timestamp_ms)` で進み、完了するまで要素を visual-dirty に保つことで、
//! 既存の dirty/フレームループ基盤（ADR-0086/0032）を再利用し、別タイマーを導入しない。

use crate::color::Color;
use crate::element::style::{Shadow, TransitionTimingValue};
use crate::element::tree::Visual;

/// トランジション中に線形補間できる連続値。
pub(crate) trait Lerp: Clone + PartialEq {
    fn lerp(&self, to: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    let t = t as f64;
    let lerp = |x: f64, y: f64| x + (y - x) * t;
    Color::new(lerp(a.r, b.r), lerp(a.g, b.g), lerp(a.b, b.b), lerp(a.a, b.a))
}

impl Lerp for Option<Color> {
    /// 片側しか設定されていなければ連続的な経路がないので、目標値へ即座にスナップする。
    fn lerp(&self, to: &Self, t: f32) -> Self {
        match (self, to) {
            (Some(a), Some(b)) => Some(lerp_color(*a, *b, t)),
            _ => *to,
        }
    }
}

impl Lerp for Vec<Shadow> {
    /// box-shadow の補間は CSS 準拠（ADR-0095）。変更前後のリストが同じ長さで全位置の
    /// `inset` フラグが一致する場合のみ各レイヤの offset/blur/spread/color を補間し、
    /// 不一致なら離散扱い（目標値を即座に採用）。
    fn lerp(&self, to: &Self, t: f32) -> Self {
        if self.len() != to.len() || self.iter().zip(to).any(|(a, b)| a.inset != b.inset) {
            return to.clone();
        }
        self.iter()
            .zip(to)
            .map(|(a, b)| Shadow {
                offset_x: a.offset_x.lerp(&b.offset_x, t),
                offset_y: a.offset_y.lerp(&b.offset_y, t),
                blur: a.blur.lerp(&b.blur, t),
                spread: a.spread.lerp(&b.spread, t),
                color: lerp_color(a.color, b.color, t),
                inset: a.inset,
            })
            .collect()
    }
}

/// 連続プロパティ1つの進行中トランジション。
#[derive(Clone, Debug)]
struct Track<T> {
    /// このカーブ開始時に表示していた値（始点）。
    from: T,
    /// カーブが向かう解決済みの値。
    target: T,
    duration_ms: f32,
    timing: TransitionTimingValue,
    /// 補間を開始したホストクロック。トリガ後の最初の `advance` が時刻を確定するまで
    /// `None`（CSS はトリガ変異時ではなく最初の観測時にトランジションを開始する）。
    start_ms: Option<f64>,
    /// 直近の `advance` によるイージング後の進捗 `[0, 1]`。
    progress: f32,
}

impl<T: Lerp> Track<T> {
    fn new(from: T, target: T, duration_ms: f32, timing: TransitionTimingValue) -> Self {
        Self {
            from,
            target,
            duration_ms,
            timing,
            start_ms: None,
            progress: 0.0,
        }
    }

    /// クロックを `now_ms` まで進める。カーブが終端に達したら `true` を返す（呼び出し側は
    /// 目標値を描画した最終フレーム後に完了トラックを破棄する）。
    fn advance(&mut self, now_ms: f64) -> bool {
        let start = *self.start_ms.get_or_insert(now_ms);
        let raw = if self.duration_ms > 0.0 {
            ((now_ms - start) as f32 / self.duration_ms).clamp(0.0, 1.0)
        } else {
            1.0
        };
        self.progress = ease(self.timing, raw);
        raw >= 1.0
    }

    /// 現在表示中の値（`from` を `target` 方向にブレンドした値）。
    fn current(&self) -> T {
        self.from.lerp(&self.target, self.progress)
    }
}

/// 1プロパティの `track` を `target` へ進め、表示する値を返す。
///
/// `prev_displayed` はこのプロパティの前フレームの画面上の値（要素の初回 emit では `None`、
/// 初期スタイルはトランジションしない）。`target` の変化は現在の表示値から連続的に方向転換
/// するため、逆方向への割り込みでも飛ばない。`duration_ms` / `timing` は変更後の解決済み
/// ビジュアルから読む。
fn step<T: Lerp>(
    track: &mut Option<Track<T>>,
    prev_displayed: Option<T>,
    target: T,
    duration_ms: f32,
    timing: TransitionTimingValue,
    now_ms: f64,
) -> T {
    let target_changed = match track {
        Some(tr) => tr.target != target,
        None => prev_displayed.as_ref().is_some_and(|p| *p != target),
    };
    if target_changed {
        if duration_ms > 0.0 {
            let from = match track {
                Some(tr) => tr.current(),
                None => prev_displayed.expect("target_changed implies a previous value"),
            };
            *track = Some(Track::new(from, target.clone(), duration_ms, timing));
        } else {
            // 変更後の duration が 0 なら即座にスナップ（CSS/DOM と同等）。
            *track = None;
        }
    }
    match track {
        Some(tr) => {
            let done = tr.advance(now_ms);
            let cur = tr.current();
            if done {
                *track = None;
            }
            cur
        }
        None => target,
    }
}

/// 1要素のプロパティ単位トランジション状態（ADR-0093）。各連続プロパティは
/// 独自の `from` と開始時刻から独立して補間する。
#[derive(Clone, Debug, Default)]
pub(crate) struct ElementTransitions {
    background_color: Option<Track<Option<Color>>>,
    border_color: Option<Track<Option<Color>>>,
    text_color: Option<Track<Option<Color>>>,
    opacity: Option<Track<f32>>,
    border_radius: Option<Track<f32>>,
    border_width: Option<Track<f32>>,
    box_shadow: Option<Track<Vec<Shadow>>>,
}

impl ElementTransitions {
    /// いずれかのプロパティがまだ補間中か。
    pub(crate) fn is_active(&self) -> bool {
        self.background_color.is_some()
            || self.border_color.is_some()
            || self.text_color.is_some()
            || self.opacity.is_some()
            || self.border_radius.is_some()
            || self.border_width.is_some()
            || self.box_shadow.is_some()
    }

    /// 変更後の解決済み `target` を前フレームの表示ビジュアルと差分し、差がある
    /// プロパティのトランジションを（再）開始して、今フレームに表示するビジュアルを返す。
    /// 離散 / enum プロパティは即座に目標値を取る。duration / timing は `target`（変更後の
    /// 解決済み実効ビジュアル）から取る。
    pub(crate) fn blend(
        &mut self,
        prev_displayed: Option<&Visual>,
        target: &Visual,
        now_ms: f64,
    ) -> Visual {
        let dur = target.transition_duration;
        let timing = target.transition_timing;
        let mut out = target.clone();
        out.background_color = step(
            &mut self.background_color,
            prev_displayed.map(|v| v.background_color),
            target.background_color,
            dur,
            timing,
            now_ms,
        );
        out.border_color = step(
            &mut self.border_color,
            prev_displayed.map(|v| v.border_color),
            target.border_color,
            dur,
            timing,
            now_ms,
        );
        out.text_color = step(
            &mut self.text_color,
            prev_displayed.map(|v| v.text_color),
            target.text_color,
            dur,
            timing,
            now_ms,
        );
        out.opacity = step(
            &mut self.opacity,
            prev_displayed.map(|v| v.opacity),
            target.opacity,
            dur,
            timing,
            now_ms,
        );
        out.border_radius = step(
            &mut self.border_radius,
            prev_displayed.map(|v| v.border_radius),
            target.border_radius,
            dur,
            timing,
            now_ms,
        );
        out.border_width = step(
            &mut self.border_width,
            prev_displayed.map(|v| v.border_width),
            target.border_width,
            dur,
            timing,
            now_ms,
        );
        out.box_shadow = step(
            &mut self.box_shadow,
            prev_displayed.map(|v| v.box_shadow.clone()),
            target.box_shadow.clone(),
            dur,
            timing,
            now_ms,
        );
        out
    }
}

/// 線形の時間比 `t` をイージング曲線で写像する。
pub(crate) fn ease(timing: TransitionTimingValue, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    match timing {
        TransitionTimingValue::Linear => t,
        // 制御点は CSS キーワードの cubic-bezier 定義に一致する。
        TransitionTimingValue::Ease => cubic_bezier_ease(0.25, 0.1, 0.25, 1.0, t),
        TransitionTimingValue::EaseIn => cubic_bezier_ease(0.42, 0.0, 1.0, 1.0, t),
        TransitionTimingValue::EaseOut => cubic_bezier_ease(0.0, 0.0, 0.58, 1.0, t),
        TransitionTimingValue::EaseInOut => cubic_bezier_ease(0.42, 0.0, 0.58, 1.0, t),
    }
}

/// CSS タイミングの cubic-bezier `(0,0) (p1x,p1y) (p2x,p2y) (1,1)` を時間比 `x` で評価する。
/// 曲線パラメータ `s` について `Bx(s) = x` を解き、`By(s)` を返す。ニュートン・ラフソン法に
/// 二分法のフォールバックを併用する（標準的な手法）。
fn cubic_bezier_ease(p1x: f32, p1y: f32, p2x: f32, p2y: f32, x: f32) -> f32 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let bezier = |a: f32, b: f32, s: f32| {
        // (1-s)^3*0 + 3(1-s)^2 s a + 3(1-s) s^2 b + s^3*1
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    let bezier_dx = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * a + 6.0 * u * s * (b - a) + 3.0 * s * s * (1.0 - b)
    };

    let mut s = x; // 初期推定値
    for _ in 0..8 {
        let x_est = bezier(p1x, p2x, s) - x;
        if x_est.abs() < 1e-5 {
            return bezier(p1y, p2y, s);
        }
        let dx = bezier_dx(p1x, p2x, s);
        if dx.abs() < 1e-6 {
            break;
        }
        s -= x_est / dx;
    }

    // 導関数が悪条件の場合の二分法フォールバック。
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    s = x;
    for _ in 0..20 {
        let x_est = bezier(p1x, p2x, s);
        if (x_est - x).abs() < 1e-5 {
            break;
        }
        if x_est < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) * 0.5;
    }
    bezier(p1y, p2y, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_identity() {
        assert_eq!(ease(TransitionTimingValue::Linear, 0.5), 0.5);
    }

    fn shadow(offset: f32, blur: f32, spread: f32, alpha: f64, inset: bool) -> Shadow {
        Shadow {
            offset_x: offset,
            offset_y: offset,
            blur,
            spread,
            color: Color::new(0.0, 0.0, 0.0, alpha),
            inset,
        }
    }

    #[test]
    fn box_shadow_interpolates_per_layer_when_length_and_inset_match() {
        let from = vec![shadow(0.0, 0.0, 0.0, 0.0, false)];
        let to = vec![shadow(10.0, 20.0, 4.0, 1.0, false)];
        let mid = from.lerp(&to, 0.5);
        assert_eq!(mid.len(), 1);
        assert!((mid[0].offset_x - 5.0).abs() < 1e-4);
        assert!((mid[0].blur - 10.0).abs() < 1e-4);
        assert!((mid[0].spread - 2.0).abs() < 1e-4);
        assert!((mid[0].color.a - 0.5).abs() < 1e-4);
        assert!(!mid[0].inset);
    }

    #[test]
    fn box_shadow_is_discrete_on_length_mismatch() {
        let from = vec![shadow(0.0, 0.0, 0.0, 1.0, false)];
        let to = vec![
            shadow(10.0, 4.0, 0.0, 1.0, false),
            shadow(2.0, 1.0, 0.0, 1.0, false),
        ];
        // トランジション途中でも目標リストへ即座にスナップする。
        assert_eq!(from.lerp(&to, 0.5), to);
    }

    #[test]
    fn box_shadow_is_discrete_on_inset_mismatch() {
        let from = vec![shadow(0.0, 0.0, 0.0, 1.0, false)];
        let to = vec![shadow(10.0, 4.0, 0.0, 1.0, true)];
        assert_eq!(from.lerp(&to, 0.5), to);
    }

    #[test]
    fn eases_pin_endpoints_and_stay_monotonic() {
        for timing in [
            TransitionTimingValue::Ease,
            TransitionTimingValue::EaseIn,
            TransitionTimingValue::EaseOut,
            TransitionTimingValue::EaseInOut,
        ] {
            assert!(ease(timing, 0.0).abs() < 1e-4);
            assert!((ease(timing, 1.0) - 1.0).abs() < 1e-4);
            let mid = ease(timing, 0.5);
            assert!(mid > 0.0 && mid < 1.0, "mid out of range for {timing:?}: {mid}");
        }
    }
}
