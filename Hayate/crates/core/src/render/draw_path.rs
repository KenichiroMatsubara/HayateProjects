//! draw display list のパス動詞列を、各 painter のパスビルダへ流し込む共有ドライバ
//! （#724 tracer の per-painter `verbs_to_path` を一本化・#726 で曲線/便宜形状/arcTo を追加）。
//!
//! painter は素の描画プリミティブ（move/line/quad/cubic/close）だけを [`PathSink`] として
//! 実装し、曲線・便宜形状（rect/rrect/oval/circle）・canvas 風 arcTo は本モジュールが
//! プリミティブへ展開する。展開が 1 箇所に集約されるので、3 painter のジオメトリは
//! ビット単位で一致する（golden は tiny-skia のみでも DOM/Canvas パリティが崩れない）。

use crate::wire::protocol::PathVerb;

/// FILL の巻き数規則（`fill_rule` enum・#726）。`nonZero` は非ゼロ巻き数、`evenOdd` は
/// 偶奇規則で、自己重なり形状の穴の有無が変わる。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawFillRule {
    NonZero,
    EvenOdd,
}

impl DrawFillRule {
    /// `DrawPaint::fill_rule`（wire は f32 の enum 値）から解決する。既定 nonZero。
    pub fn from_wire(raw: f32) -> Self {
        if raw as u32 == 1 {
            Self::EvenOdd
        } else {
            Self::NonZero
        }
    }
}

/// 2×3 アフィン変換 `[a, b, c, d, e, f]`（#728）。点 `(x, y)` を
/// `(a·x + c·y + e, b·x + d·y + f)` へ写す（CSS / canvas の matrix(a,b,c,d,e,f) と同順）。
/// draw の座標操作（translate / rotate / scale / transform）を walk 側で verbs に
/// ソフト適用するために使う。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2(pub [f32; 6]);

impl Affine2 {
    pub const IDENTITY: Affine2 = Affine2([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);

    pub fn translate(tx: f32, ty: f32) -> Self {
        Affine2([1.0, 0.0, 0.0, 1.0, tx, ty])
    }

    pub fn scale(sx: f32, sy: f32) -> Self {
        Affine2([sx, 0.0, 0.0, sy, 0.0, 0.0])
    }

    pub fn rotate(radians: f32) -> Self {
        let (s, c) = radians.sin_cos();
        Affine2([c, s, -s, c, 0.0, 0.0])
    }

    /// 点を写す。
    pub fn apply(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, c, d, e, f] = self.0;
        (a * x + c * y + e, b * x + d * y + f)
    }

    /// `self` の後に `rhs` を適用する合成（`apply(self.then(rhs), p) == self.apply(rhs.apply(p))`）。
    /// canvas の translate/rotate/scale は現在の CTM に**後置**で積むので `ctm = ctm.then(op)`。
    pub fn then(self, rhs: Affine2) -> Affine2 {
        let [a, b, c, d, e, f] = self.0;
        let [a2, b2, c2, d2, e2, f2] = rhs.0;
        Affine2([
            a * a2 + c * b2,
            b * a2 + d * b2,
            a * c2 + c * d2,
            b * c2 + d * d2,
            a * e2 + c * f2 + e,
            b * e2 + d * f2 + f,
        ])
    }

    /// 面積スケール係数 `sqrt(|det|)`。stroke 幅を近似的に変換へ追従させるのに使う。
    pub fn scale_factor(&self) -> f32 {
        let [a, b, c, d, _, _] = self.0;
        (a * d - b * c).abs().sqrt()
    }
}

/// verbs をアフィン変換し、プリミティブ（move/line/quad/cubic/close）の PathVerb 列へ
/// 平坦化して集める PathSink（#728）。便宜形状・arcTo は [`build_draw_path`] が cubic へ
/// 展開してから変換されるので、回転下でも正しい（矩形が回転した四辺形になる）。
struct VerbTransformer {
    m: Affine2,
    out: Vec<PathVerb>,
}

impl PathSink for VerbTransformer {
    fn move_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.m.apply(x, y);
        self.out.push(PathVerb::MoveTo { x, y });
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let (x, y) = self.m.apply(x, y);
        self.out.push(PathVerb::LineTo { x, y });
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        let (cx, cy) = self.m.apply(cx, cy);
        let (x, y) = self.m.apply(x, y);
        self.out.push(PathVerb::QuadraticTo { cx, cy, x, y });
    }
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        let (c1x, c1y) = self.m.apply(c1x, c1y);
        let (c2x, c2y) = self.m.apply(c2x, c2y);
        let (x, y) = self.m.apply(x, y);
        self.out.push(PathVerb::CubicTo {
            c1x,
            c1y,
            c2x,
            c2y,
            x,
            y,
        });
    }
    fn close(&mut self) {
        self.out.push(PathVerb::Close);
    }
}

/// `verbs` をアフィン `m` で写した、プリミティブのみの PathVerb 列を返す（#728）。
pub fn transform_verbs(verbs: &[PathVerb], m: Affine2) -> Vec<PathVerb> {
    let mut t = VerbTransformer {
        m,
        out: Vec::new(),
    };
    build_draw_path(verbs, &mut t);
    t.out
}

/// stroke の線端（`line_cap` enum・#727）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawLineCap {
    Butt,
    Round,
    Square,
}

impl DrawLineCap {
    pub fn from_wire(raw: f32) -> Self {
        match raw as u32 {
            1 => Self::Round,
            2 => Self::Square,
            _ => Self::Butt,
        }
    }
}

/// stroke の頂点の継ぎ方（`line_join` enum・#727）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawLineJoin {
    Miter,
    Round,
    Bevel,
}

impl DrawLineJoin {
    pub fn from_wire(raw: f32) -> Self {
        match raw as u32 {
            1 => Self::Round,
            2 => Self::Bevel,
            _ => Self::Miter,
        }
    }
}

/// STROKE が描く輪郭のスタイル（#727）。painter に渡す前に `DrawPaint` の wire 値
/// （enum は f32）を意味型へ解決したもの。
#[derive(Debug, Clone, PartialEq)]
pub struct StrokeStyle {
    pub width: f32,
    pub cap: DrawLineCap,
    pub join: DrawLineJoin,
    pub miter_limit: f32,
    /// on/off 間隔の並び（論理 px）。空なら実線。
    pub dash: Vec<f32>,
    pub dash_offset: f32,
}

impl StrokeStyle {
    /// tagged paint packet（`DrawPaint`）の stroke フィールドから解決する。
    pub fn from_paint(paint: &crate::wire::protocol::DrawPaint) -> Self {
        Self {
            width: paint.stroke_width,
            cap: DrawLineCap::from_wire(paint.cap),
            join: DrawLineJoin::from_wire(paint.join),
            miter_limit: paint.miter_limit,
            dash: paint.dash.clone(),
            dash_offset: paint.dash_offset,
        }
    }
}

/// painter 側パスビルダの最小サーフェス。各 painter は自前のパス型
/// （tiny-skia `PathBuilder` / kurbo `BezPath`）にこの 5 プリミティブだけを橋渡しし、
/// 曲線・便宜形状・arcTo の展開は [`build_draw_path`] に任せる。
pub trait PathSink {
    fn move_to(&mut self, x: f32, y: f32);
    fn line_to(&mut self, x: f32, y: f32);
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32);
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32);
    fn close(&mut self);
}

/// 円弧を 90° 以下の 3 次ベジェで近似するときの制御点係数（kappa = 4/3·tan(π/8)）。
const KAPPA: f32 = 0.552_284_75;

/// パス動詞列を `sink` へ流す（ボーダーボックス相対座標のまま）。
///
/// `MoveTo` で開いた subpath の中でだけ `LineTo` / `Close` / 曲線動詞を受け付ける
/// （退化列は黙って捨てる。#724 からの 3 painter 共通の意味論）。便宜形状
/// （rect/rrect/oval/circle）は自前で subpath を開閉するので開いていなくてもよい。
pub fn build_draw_path<S: PathSink>(verbs: &[PathVerb], sink: &mut S) {
    let mut open = false;
    // 現在点。arcTo の先頭接線・曲線の始点に要る。
    let mut cur = (0.0_f32, 0.0_f32);
    for verb in verbs {
        match verb {
            PathVerb::MoveTo { x, y } => {
                sink.move_to(*x, *y);
                cur = (*x, *y);
                open = true;
            }
            PathVerb::LineTo { x, y } if open => {
                sink.line_to(*x, *y);
                cur = (*x, *y);
            }
            PathVerb::QuadraticTo { cx, cy, x, y } if open => {
                sink.quad_to(*cx, *cy, *x, *y);
                cur = (*x, *y);
            }
            PathVerb::CubicTo {
                c1x,
                c1y,
                c2x,
                c2y,
                x,
                y,
            } if open => {
                sink.cubic_to(*c1x, *c1y, *c2x, *c2y, *x, *y);
                cur = (*x, *y);
            }
            PathVerb::ArcTo {
                x1,
                y1,
                x2,
                y2,
                radius,
            } if open => {
                cur = append_arc_to(sink, cur, (*x1, *y1), (*x2, *y2), *radius);
            }
            PathVerb::Rect {
                x,
                y,
                width,
                height,
            } => {
                append_rect(sink, *x, *y, *width, *height);
                cur = (*x, *y);
                open = true;
            }
            PathVerb::Rrect {
                x,
                y,
                width,
                height,
                rx,
                ry,
            } => {
                append_rrect(sink, *x, *y, *width, *height, *rx, *ry);
                cur = (*x, *y);
                open = true;
            }
            PathVerb::Oval {
                x,
                y,
                width,
                height,
            } => {
                append_oval(sink, *x, *y, *width, *height);
                cur = (*x + *width, *y + *height * 0.5);
                open = true;
            }
            PathVerb::Circle { cx, cy, radius } => {
                append_oval(sink, *cx - *radius, *cy - *radius, *radius * 2.0, *radius * 2.0);
                cur = (*cx + *radius, *cy);
                open = true;
            }
            // MoveTo 前の LineTo / Close / 曲線は退化列として捨てる。
            PathVerb::LineTo { .. }
            | PathVerb::QuadraticTo { .. }
            | PathVerb::CubicTo { .. }
            | PathVerb::ArcTo { .. } => {}
            PathVerb::Close => {
                if open {
                    sink.close();
                    open = false;
                }
            }
        }
    }
}

/// 矩形の閉 subpath を追加する。
fn append_rect<S: PathSink>(sink: &mut S, x: f32, y: f32, w: f32, h: f32) {
    sink.move_to(x, y);
    sink.line_to(x + w, y);
    sink.line_to(x + w, y + h);
    sink.line_to(x, y + h);
    sink.close();
}

/// 角丸（楕円コーナー）矩形の閉 subpath を追加する。半径は各辺の半分でクランプする。
fn append_rrect<S: PathSink>(sink: &mut S, x: f32, y: f32, w: f32, h: f32, rx: f32, ry: f32) {
    let rx = rx.clamp(0.0, w * 0.5);
    let ry = ry.clamp(0.0, h * 0.5);
    if rx <= 0.0 || ry <= 0.0 {
        append_rect(sink, x, y, w, h);
        return;
    }
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    sink.move_to(x + rx, y);
    sink.line_to(x + w - rx, y);
    sink.cubic_to(x + w - rx + kx, y, x + w, y + ry - ky, x + w, y + ry);
    sink.line_to(x + w, y + h - ry);
    sink.cubic_to(x + w, y + h - ry + ky, x + w - rx + kx, y + h, x + w - rx, y + h);
    sink.line_to(x + rx, y + h);
    sink.cubic_to(x + rx - kx, y + h, x, y + h - ry + ky, x, y + h - ry);
    sink.line_to(x, y + ry);
    sink.cubic_to(x, y + ry - ky, x + rx - kx, y, x + rx, y);
    sink.close();
}

/// (x, y, w, h) に内接する楕円の閉 subpath を、右中央を始点に 4 本の 3 次ベジェで追加する。
fn append_oval<S: PathSink>(sink: &mut S, x: f32, y: f32, w: f32, h: f32) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let rx = w * 0.5;
    let ry = h * 0.5;
    let cx = x + rx;
    let cy = y + ry;
    let kx = rx * KAPPA;
    let ky = ry * KAPPA;
    sink.move_to(cx + rx, cy);
    sink.cubic_to(cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry);
    sink.cubic_to(cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy);
    sink.cubic_to(cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry);
    sink.cubic_to(cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy);
    sink.close();
}

/// HTML canvas 風 arcTo。現在点 `p0` から `p1` へ向かう線と `p1`→`p2` の線に接する半径
/// `radius` の円弧を追加する。始点接点までを直線で結ぶ。半径 0・collinear・退化は
/// `p1` への lineTo に退化する。円弧終点（次の現在点）を返す。
fn append_arc_to<S: PathSink>(
    sink: &mut S,
    p0: (f32, f32),
    p1: (f32, f32),
    p2: (f32, f32),
    radius: f32,
) -> (f32, f32) {
    let eps = 1e-6_f32;
    let (x0, y0) = p0;
    let (x1, y1) = p1;
    let (x2, y2) = p2;
    let (d0x, d0y) = (x0 - x1, y0 - y1);
    let (d1x, d1y) = (x2 - x1, y2 - y1);
    let len0 = (d0x * d0x + d0y * d0y).sqrt();
    let len1 = (d1x * d1x + d1y * d1y).sqrt();
    if radius <= eps || len0 < eps || len1 < eps {
        sink.line_to(x1, y1);
        return (x1, y1);
    }
    let u0 = (d0x / len0, d0y / len0);
    let u1 = (d1x / len1, d1y / len1);
    let cos_theta = (u0.0 * u1.0 + u0.1 * u1.1).clamp(-1.0, 1.0);
    let theta = cos_theta.acos();
    let tan_half = (theta * 0.5).tan();
    if tan_half.abs() < eps {
        // collinear（同方向 / 逆方向）は円弧が定義できない → lineTo。
        sink.line_to(x1, y1);
        return (x1, y1);
    }
    let dist = radius / tan_half;
    // 接点（p1 から各辺方向へ dist）。
    let t0 = (x1 + u0.0 * dist, y1 + u0.1 * dist);
    let t1 = (x1 + u1.0 * dist, y1 + u1.1 * dist);
    // 中心は二等分線方向へ radius/sin(θ/2)。
    let bis = (u0.0 + u1.0, u0.1 + u1.1);
    let bis_len = (bis.0 * bis.0 + bis.1 * bis.1).sqrt();
    if bis_len < eps {
        sink.line_to(x1, y1);
        return (x1, y1);
    }
    let sin_half = (theta * 0.5).sin();
    let center_dist = radius / sin_half;
    let center = (
        x1 + bis.0 / bis_len * center_dist,
        y1 + bis.1 / bis_len * center_dist,
    );
    let a0 = (t0.1 - center.1).atan2(t0.0 - center.0);
    let a1 = (t1.1 - center.1).atan2(t1.0 - center.0);
    sink.line_to(t0.0, t0.1);
    append_arc(sink, center.0, center.1, radius, a0, a1);
    t1
}

/// 中心 `(cx, cy)`・半径 `r` の円弧を `a0`→`a1`（ラジアン）で 3 次ベジェ列として追加する。
/// 最短方向へ回り、各セグメントは 90° 以下。始点 `(a0)` へは呼び出し側が既に到達している
/// 前提（lineTo 済み）。
fn append_arc<S: PathSink>(sink: &mut S, cx: f32, cy: f32, r: f32, a0: f32, mut a1: f32) {
    use std::f32::consts::PI;
    // 最短方向の掃引角へ正規化（-π..π）。
    let mut sweep = a1 - a0;
    while sweep > PI {
        sweep -= 2.0 * PI;
    }
    while sweep < -PI {
        sweep += 2.0 * PI;
    }
    if sweep.abs() < 1e-6 {
        return;
    }
    a1 = a0 + sweep;
    let segments = (sweep.abs() / (PI * 0.5)).ceil().max(1.0) as usize;
    let delta = sweep / segments as f32;
    let mut theta = a0;
    for _ in 0..segments {
        let next = theta + delta;
        // 単位円弧の制御点長さ。
        let alpha = 4.0 / 3.0 * (delta * 0.25).tan();
        let (s0, c0) = theta.sin_cos();
        let (s1, c1) = next.sin_cos();
        let p1x = cx + r * (c0 - alpha * s0);
        let p1y = cy + r * (s0 + alpha * c0);
        let p2x = cx + r * (c1 + alpha * s1);
        let p2y = cy + r * (s1 - alpha * c1);
        let ex = cx + r * c1;
        let ey = cy + r * s1;
        sink.cubic_to(p1x, p1y, p2x, p2y, ex, ey);
        theta = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// プリミティブ呼び出しを記録する PathSink。
    #[derive(Default)]
    struct Recorder {
        calls: Vec<String>,
        last: (f32, f32),
    }

    impl PathSink for Recorder {
        fn move_to(&mut self, x: f32, y: f32) {
            self.calls.push(format!("M {x} {y}"));
            self.last = (x, y);
        }
        fn line_to(&mut self, x: f32, y: f32) {
            self.calls.push(format!("L {x} {y}"));
            self.last = (x, y);
        }
        fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
            self.calls.push(format!("Q {cx} {cy} {x} {y}"));
            self.last = (x, y);
        }
        fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
            self.calls.push(format!("C {c1x} {c1y} {c2x} {c2y} {x} {y}"));
            self.last = (x, y);
        }
        fn close(&mut self) {
            self.calls.push("Z".to_string());
        }
    }

    fn run(verbs: &[PathVerb]) -> Recorder {
        let mut r = Recorder::default();
        build_draw_path(verbs, &mut r);
        r
    }

    #[test]
    fn drops_degenerate_verbs_before_move_to() {
        let r = run(&[
            PathVerb::LineTo { x: 1.0, y: 1.0 },
            PathVerb::Close,
            PathVerb::CubicTo {
                c1x: 0.0,
                c1y: 0.0,
                c2x: 0.0,
                c2y: 0.0,
                x: 2.0,
                y: 2.0,
            },
        ]);
        assert!(r.calls.is_empty(), "verbs before MoveTo are dropped: {:?}", r.calls);
    }

    #[test]
    fn forwards_curves_after_move_to() {
        let r = run(&[
            PathVerb::MoveTo { x: 0.0, y: 0.0 },
            PathVerb::QuadraticTo {
                cx: 5.0,
                cy: 10.0,
                x: 10.0,
                y: 0.0,
            },
            PathVerb::CubicTo {
                c1x: 12.0,
                c1y: 2.0,
                c2x: 14.0,
                c2y: 8.0,
                x: 16.0,
                y: 0.0,
            },
        ]);
        assert_eq!(
            r.calls,
            vec!["M 0 0", "Q 5 10 10 0", "C 12 2 14 8 16 0"],
        );
    }

    #[test]
    fn rect_emits_closed_polygon() {
        let r = run(&[PathVerb::Rect {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        }]);
        assert_eq!(
            r.calls,
            vec!["M 1 2", "L 4 2", "L 4 6", "L 1 6", "Z"],
        );
    }

    #[test]
    fn circle_is_a_closed_four_arc_oval() {
        let r = run(&[PathVerb::Circle {
            cx: 10.0,
            cy: 10.0,
            radius: 5.0,
        }]);
        // 右中央 (15,10) を始点、4 本の cubic、close。
        assert_eq!(r.calls.first().unwrap(), "M 15 10");
        assert_eq!(r.calls.iter().filter(|c| c.starts_with('C')).count(), 4);
        assert_eq!(r.calls.last().unwrap(), "Z");
    }

    #[test]
    fn arc_to_starts_with_leading_line_to_tangent_point() {
        // (0,0) -> corner (10,0) -> (10,10), radius 5。第 1 接点は (5,0)。
        let r = run(&[
            PathVerb::MoveTo { x: 0.0, y: 0.0 },
            PathVerb::ArcTo {
                x1: 10.0,
                y1: 0.0,
                x2: 10.0,
                y2: 10.0,
                radius: 5.0,
            },
        ]);
        assert_eq!(r.calls[0], "M 0 0");
        assert_eq!(r.calls[1], "L 5 0", "leading line to the first tangent point");
        // 円弧は cubic で近似され、終点は第 2 接点 (10,5) 付近。
        assert!(r.calls[2].starts_with('C'));
        assert!((r.last.0 - 10.0).abs() < 1e-3 && (r.last.1 - 5.0).abs() < 1e-3, "ends at (10,5), got {:?}", r.last);
    }

    #[test]
    fn arc_to_degenerates_to_line_when_radius_zero() {
        let r = run(&[
            PathVerb::MoveTo { x: 0.0, y: 0.0 },
            PathVerb::ArcTo {
                x1: 10.0,
                y1: 0.0,
                x2: 10.0,
                y2: 10.0,
                radius: 0.0,
            },
        ]);
        assert_eq!(r.calls, vec!["M 0 0", "L 10 0"]);
    }
}
