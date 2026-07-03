use hayate_core::{
    RenderImage, RenderImageAlphaType, ScenePainter, ShadowOccluder, TextRunData, is_notdef,
    missing_glyph_placeholder,
};
use skrifa::{
    GlyphId, MetadataProvider,
    instance::{LocationRef, NormalizedCoord, Size},
    outline::{DrawSettings, OutlinePen},
    raw::FontRef,
};
use tiny_skia::{
    Color, FillRule, LineCap, LineJoin, Mask, Paint, Path, PathBuilder, Pixmap,
    PixmapPaint, PixmapRef, PremultipliedColorU8, Stroke, Transform,
};

fn normalized_coords_ref(coords: &[i16]) -> &[NormalizedCoord] {
    // Parley は harfrust/skrifa の正規化座標を i16(F2Dot14)で保持する。
    unsafe { std::slice::from_raw_parts(coords.as_ptr().cast(), coords.len()) }
}

use crate::straight_to_premultiplied;

struct PainterState {
    transform: Transform,
    transform_stack: Vec<Transform>,
    clip_masks: Vec<Mask>,
}

pub struct TinySkiaPainter<'a> {
    pixmap: &'a mut Pixmap,
    state: PainterState,
}

impl<'a> TinySkiaPainter<'a> {
    pub fn new(pixmap: &'a mut Pixmap, content_scale: f32) -> Self {
        let transform = if content_scale == 1.0 {
            Transform::identity()
        } else {
            Transform::from_scale(content_scale, content_scale)
        };
        Self {
            pixmap,
            state: PainterState {
                transform,
                transform_stack: Vec::new(),
                clip_masks: Vec::new(),
            },
        }
    }

}

impl ScenePainter for TinySkiaPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        let pixmap = &mut self.pixmap;
        let paint = color_to_paint(color);
        if corner_radius == 0.0 {
            if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
                pixmap.fill_rect(rect, &paint, transform, mask);
            }
        } else if let Some(path) = rounded_rect_path(x, y, width, height, corner_radius) {
            pixmap.fill_path(&path, &paint, FillRule::Winding, transform, mask);
        }
    }

    fn fill_rounded_ring(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    ) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        let pixmap = &mut self.pixmap;
        let bw = border_width.max(0.0);
        let inner_w = (width - 2.0 * bw).max(0.0);
        let inner_h = (height - 2.0 * bw).max(0.0);
        if inner_w <= 0.0 || inner_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        // リング帯だけを 1 回の even-odd フィル(外側マイナス内側)で塗る。内側を
        // `BlendMode::Clear` でくり抜くと下の不透明コンテンツを消してしまう — 例えば
        // ネイティブフォーカスリングは塗り済み input の上に乗るため、透明化しては
        // ならない。vello バックエンドの even-odd 帯フィルと同じ。
        let paint = color_to_paint(color);
        let inner_r = (outer_radius - bw).max(0.0);
        let mut pb = PathBuilder::new();
        push_rounded_rect(&mut pb, x, y, width, height, outer_radius);
        push_rounded_rect(&mut pb, x + bw, y + bw, inner_w, inner_h, inner_r);
        if let Some(path) = pb.finish() {
            pixmap.fill_path(&path, &paint, FillRule::EvenOdd, transform, mask);
        }
    }

    fn fill_blurred_rounded_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        corner_radius: f32,
        std_dev: f32,
        color: [f32; 4],
        occluder: Option<ShadowOccluder>,
    ) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        draw_blurred_rounded_rect(
            &mut self.pixmap,
            x,
            y,
            width,
            height,
            corner_radius,
            std_dev,
            color,
            occluder,
            transform,
            mask,
        );
    }

    fn stroke_dashed_border(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        outer_radius: f32,
        border_width: f32,
        color: [f32; 4],
    ) {
        let bw = border_width.max(0.0);
        if bw <= 0.0 || width <= 0.0 || height <= 0.0 {
            return;
        }
        let half = bw / 2.0;
        let inset_w = width - bw;
        let inset_h = height - bw;
        // ボックスより太いボーダーはソリッドフィルに退化する。
        if inset_w <= 0.0 || inset_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }

        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        let inner_r = (outer_radius - half).max(0.0);
        let Some(path) = rounded_rect_path(x + half, y + half, inset_w, inset_h, inner_r) else {
            return;
        };
        let paint = color_to_paint(color);
        let dash = bw * 2.0;
        let mut stroke = Stroke {
            width: bw,
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            ..Stroke::default()
        };
        stroke.dash = tiny_skia::StrokeDash::new(vec![dash, dash], 0.0);
        self.pixmap
            .stroke_path(&path, &paint, &stroke, transform, mask);
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        draw_text_run(&mut self.pixmap, x, y, color, data, transform, mask);
    }

    fn draw_image(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        data: &RenderImage,
    ) {
        let transform = self.state.transform;
        let mask = self.state.clip_masks.last();
        draw_image(
            &mut self.pixmap,
            x,
            y,
            width,
            height,
            data,
            transform,
            mask,
        );
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        self.state.transform_stack.push(self.state.transform);
        let [a, b, c, d, e, f] = transform;
        let group_ts =
            Transform::from_row(a as f32, b as f32, c as f32, d as f32, e as f32, f as f32);
        self.state.transform = self.state.transform.pre_concat(group_ts);
    }

    fn pop_transform(&mut self) {
        if let Some(previous) = self.state.transform_stack.pop() {
            self.state.transform = previous;
        }
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        let transform = self.state.transform;
        // 一様な半径(現状 Hayate が出す唯一の形状)。0 なら矩形クリップ。
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        let path = if radius > 0.0 {
            rounded_rect_path(x, y, width, height, radius)
        } else if let Some(rect) = tiny_skia::Rect::from_xywh(x, y, width, height) {
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            pb.finish()
        } else {
            None
        };
        if let Some(path) = path {
            match self.state.clip_masks.last() {
                Some(parent) => {
                    let mut clip_mask = parent.clone();
                    clip_mask.intersect_path(&path, FillRule::Winding, true, transform);
                    self.state.clip_masks.push(clip_mask);
                }
                None => {
                    if let Some(mut clip_mask) =
                        Mask::new(self.pixmap.width(), self.pixmap.height())
                    {
                        clip_mask.fill_path(&path, FillRule::Winding, true, transform);
                        self.state.clip_masks.push(clip_mask);
                    }
                }
            }
        }
    }

    fn pop_clip(&mut self) {
        self.state.clip_masks.pop();
    }
}

// ── 解析ぼかしシャドウ（issue #658）─────────────────────────────────────────
// ぼかし角丸矩形（[`hayate_core::NodeKind::BlurredRoundedRect`]）を、default の erf シェル
// 近似フォールバックではなく **per-pixel の解析被覆**で塗る。ガウス × 角丸矩形の閉形式
// （Raph Levien の近似・vello の `draw_blurred_rounded_rect` シェーダと同一式）を影 bbox 内の
// ピクセルだけ走査して評価するので、2 レンダラ間の DOM/Canvas パリティ（ADR-0102）が保たれる。
// ref: https://raphlinus.github.io/graphics/2020/04/21/blurred-rounded-rects.html

/// ガウスの裾を打ち切る半径（σ の倍数）。影 bbox はこの分だけ外形から膨らませ、応答が実質 0 の
/// 遠方ピクセルは走査しない（コスト tunable）。
const SHADOW_BLUR_KERNEL_SIGMAS: f32 = 2.5;
/// 外形から内側へこの距離（σ の倍数）より深いピクセルは被覆 ≈ 1 に飽和するので、閉形式の
/// 超越関数評価を省いて 1.0 に短絡する（大きなボックスで per-pixel コストを falloff 帯だけに抑える
/// 主要最適化・コスト tunable）。3σ で erf の裾は < 0.1% なので継ぎ目は u8 丸めに埋もれる。
const SHADOW_BLUR_INTERIOR_SIGMAS: f32 = 3.0;
/// 角丸半径をぼかしへブレンドする内側係数（Raph 近似の r0）。
const SHADOW_BLUR_RADIUS_INNER_SIGMA: f32 = 1.15;
/// 角丸半径をぼかしへブレンドする外側係数（Raph 近似の r1）。
const SHADOW_BLUR_RADIUS_OUTER_SIGMA: f32 = 2.0;
/// 長辺を内側へ引き込み矩形の偏心を弱める係数（Raph 近似の delta）。
const SHADOW_BLUR_ECCENTRICITY: f32 = 1.25;

/// 誤差関数 erf(x) の近似（Raph Levien、vello `erf7` と同一）。pow(x,14) まで計算するため
/// overflow を防ぐようクランプする。
fn erf7(x: f32) -> f32 {
    let y = (x * 1.128_379_2).clamp(-100.0, 100.0);
    let yy = y * y;
    let z = y + (0.24295 + (0.03395 + 0.0104 * yy) * yy) * (y * yy);
    z / (1.0 + z * z).sqrt()
}

/// ガウス × 角丸矩形の畳み込み被覆（0..1）を局所座標 `(lx, ly)`（矩形中心が原点）で評価する。
/// `const` 引数は影ごとに一度計算した定数（per-pixel ループの外で用意する）。
#[allow(clippy::too_many_arguments)]
fn blurred_rrect_coverage(
    lx: f32,
    ly: f32,
    adj_w: f32,
    adj_h: f32,
    min_edge: f32,
    r1: f32,
    exponent: f32,
    inv_exponent: f32,
    inv_std_dev: f32,
    scale: f32,
) -> f32 {
    let y0 = ly.abs() - (adj_h * 0.5 - r1);
    let x0 = lx.abs() - (adj_w * 0.5 - r1);
    let x1 = x0.max(0.0);
    let y1 = y0.max(0.0);
    // 超越 `powf` は角（x1>0 かつ y1>0）だけで要る。直線縁ではどちらかが 0 なので
    // `pow(v^e, 1/e) = v` に退化し、帯の大多数を占める縁ピクセルで powf を丸ごと省ける。
    let d_pos = if x1 == 0.0 {
        y1
    } else if y1 == 0.0 {
        x1
    } else {
        (x1.powf(exponent) + y1.powf(exponent)).powf(inv_exponent)
    };
    let d_neg = x0.max(y0).min(0.0);
    let d = d_pos + d_neg - r1;
    // 反対の縁への寄与 `erf7(inv·(min_edge+d))` は、`inv·(min_edge+d)` が大きいと 1.0 に飽和する
    // （大きなボックスの帯では常にそう）。閾値超えは erf7 を 1 回省く。
    let arg_hi = inv_std_dev * (min_edge + d);
    let e_hi = if arg_hi > 3.0 { 1.0 } else { erf7(arg_hi) };
    let alpha = scale * (e_hi - erf7(inv_std_dev * d));
    alpha.clamp(0.0, 1.0)
}

/// ぼかし角丸矩形の影を解析被覆で影 bbox に塗る。`(x, y, width, height)` は影外形
/// （オフセット・spread 適用済み）、`corner_radius` はその角丸半径、`std_dev` はガウス σ、
/// `color` は影色（straight RGBA・不透明度適用済み）。被覆を premultiplied な小 pixmap に焼き、
/// `draw_pixmap` で transform / clip マスク越しに合成する（`draw_image` と同経路）。
#[allow(clippy::too_many_arguments)]
fn draw_blurred_rounded_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    corner_radius: f32,
    std_dev: f32,
    color: [f32; 4],
    occluder: Option<ShadowOccluder>,
    transform: Transform,
    mask: Option<&Mask>,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let std_dev = std_dev.max(1e-5);
    let inv_std_dev = 1.0 / std_dev;

    // 影ごとに一度だけ計算する形状定数（vello シェーダと同順）。
    let min_edge = width.min(height);
    let radius_max = 0.5 * min_edge;
    let radius = corner_radius.max(0.0);
    let r0 = (radius * radius + (std_dev * SHADOW_BLUR_RADIUS_INNER_SIGMA).powi(2))
        .sqrt()
        .min(radius_max);
    let r1 = (radius * radius + (std_dev * SHADOW_BLUR_RADIUS_OUTER_SIGMA).powi(2))
        .sqrt()
        .min(radius_max);
    let exponent = 2.0 * r1 / r0;
    let inv_exponent = 1.0 / exponent;
    // 長辺を引き込み偏心を弱める（delta は元の width/height から求める）。
    let delta = SHADOW_BLUR_ECCENTRICITY
        * std_dev
        * ((-(0.5 * inv_std_dev * width).powi(2)).exp()
            - (-(0.5 * inv_std_dev * height).powi(2)).exp());
    let adj_w = width + delta.min(0.0);
    let adj_h = height - delta.max(0.0);
    let scale = 0.5 * erf7(inv_std_dev * 0.5 * (adj_w.max(adj_h) - 0.5 * radius));

    let cx = x + width * 0.5;
    let cy = y + height * 0.5;

    // 被覆 ≈ 1 に飽和する内側矩形の半外形（この内側は超越関数を評価せず 1.0 に短絡する）。
    let interior = SHADOW_BLUR_INTERIOR_SIGMAS * std_dev;
    let interior_hx = width * 0.5 - interior;
    let interior_hy = height * 0.5 - interior;

    // 局所座標 `(lx, ly)`（矩形中心が原点）の被覆。外形から十分内側なら閉形式を評価せず
    // 飽和値 1.0 に短絡する（大きなボックスで per-pixel コストを falloff 帯だけに抑える）。
    let coverage = move |lx: f32, ly: f32| -> f32 {
        if ly.abs() <= interior_hy && lx.abs() <= interior_hx {
            1.0
        } else {
            blurred_rrect_coverage(
                lx, ly, adj_w, adj_h, min_edge, r1, exponent, inv_exponent, inv_std_dev, scale,
            )
        }
    };

    // ガウスの裾ぶんだけ外形から膨らませた影 bbox（シーン座標）。
    let pad = (SHADOW_BLUR_KERNEL_SIGMAS * std_dev).ceil();
    let bx0 = (x - pad).floor();
    let by0 = (y - pad).floor();
    let bx1 = (x + width + pad).ceil();
    let by1 = (y + height + pad).ceil();
    if bx1 <= bx0 || by1 <= by0 {
        return;
    }

    // 回転・スキュー・非正スケールが無ければ（Hayate の実利用: content_scale ＋ scroll 平行移動）、
    // 一時 pixmap を介さず対象 pixmap へ直接 1 パス合成する——大きな影 bbox の alloc＋blit を避け、
    // かつ画面外の帯は反転写像で clamp して走査しない。それ以外の affine は一時 pixmap にフォールバック。
    let no_skew = transform.kx == 0.0
        && transform.ky == 0.0
        && transform.sx > 0.0
        && transform.sy > 0.0;
    if no_skew {
        composite_blur_direct(
            pixmap, mask, transform, bx0, by0, bx1, by1, cx, cy, color, occluder, coverage,
        );
    } else {
        composite_blur_via_temp(
            pixmap, mask, transform, bx0, by0, bx1, by1, cx, cy, color, occluder, coverage,
        );
    }
}

/// ぼかしシャドウの被覆を対象 pixmap へ直接 SrcOver 合成する（回転/スキュー無しの affine 用）。
/// device 空間の bbox を pixmap 内へ clamp し、各対象ピクセル中心を逆写像して局所座標の被覆を
/// 評価するので、一時バッファも余分なメモリ往復も無く、画面外は走査しない。
#[allow(clippy::too_many_arguments)]
fn composite_blur_direct(
    pixmap: &mut Pixmap,
    mask: Option<&Mask>,
    transform: Transform,
    bx0: f32,
    by0: f32,
    bx1: f32,
    by1: f32,
    cx: f32,
    cy: f32,
    color: [f32; 4],
    occluder: Option<ShadowOccluder>,
    coverage: impl Fn(f32, f32) -> f32,
) {
    let [cr, cg, cb, ca] = color;
    let (sx, sy, tx, ty) = (transform.sx, transform.sy, transform.tx, transform.ty);
    let inv_sx = 1.0 / sx;
    let inv_sy = 1.0 / sy;
    let pw = pixmap.width() as i64;
    let ph = pixmap.height() as i64;
    let dx0 = ((sx * bx0 + tx).floor() as i64).max(0);
    let dy0 = ((sy * by0 + ty).floor() as i64).max(0);
    let dx1 = ((sx * bx1 + tx).ceil() as i64).min(pw);
    let dy1 = ((sy * by1 + ty).ceil() as i64).min(ph);
    if dx0 >= dx1 || dy0 >= dy1 {
        return;
    }
    let (mask_data, mask_w, mask_h) = match mask {
        Some(m) => (Some(m.data()), m.width() as i64, m.height() as i64),
        None => (None, 0, 0),
    };
    let pw_us = pw as usize;
    let pixels = pixmap.pixels_mut();
    for dy in dy0..dy1 {
        let scene_y = (dy as f32 + 0.5 - ty) * inv_sy;
        let ly = scene_y - cy;
        let row = dy as usize * pw_us;
        for dx in dx0..dx1 {
            let scene_x = (dx as f32 + 0.5 - tx) * inv_sx;
            let lx = scene_x - cx;
            // 不透明 owner に覆われる内側は描かない（issue #659。覆われて見えない overdraw を省く）。
            if let Some(occ) = occluder {
                if occ.contains(scene_x, scene_y) {
                    continue;
                }
            }
            let cov = coverage(lx, ly);
            if cov <= 0.0 {
                continue;
            }
            let mut a = ca * cov;
            if let Some(md) = mask_data {
                if dx >= mask_w || dy >= mask_h {
                    continue;
                }
                a *= md[(dy * mask_w + dx) as usize] as f32 / 255.0;
                if a <= 0.0 {
                    continue;
                }
            }
            let idx = row + dx as usize;
            let dst = pixels[idx];
            let inv_a = 1.0 - a;
            let out_r = cr * a + dst.red() as f32 / 255.0 * inv_a;
            let out_g = cg * a + dst.green() as f32 / 255.0 * inv_a;
            let out_b = cb * a + dst.blue() as f32 / 255.0 * inv_a;
            let out_a = a + dst.alpha() as f32 / 255.0 * inv_a;
            pixels[idx] = PremultipliedColorU8::from_rgba(
                (out_r * 255.0 + 0.5) as u8,
                (out_g * 255.0 + 0.5) as u8,
                (out_b * 255.0 + 0.5) as u8,
                (out_a * 255.0 + 0.5) as u8,
            )
            .unwrap_or(dst);
        }
    }
}

/// ぼかしシャドウを一時 pixmap に焼き `draw_pixmap` で合成する（回転/スキューを含む一般 affine 用）。
/// tiny-skia の変換・クリップ経路をそのまま使うが、bbox 全面の alloc＋blit を伴う（直接合成が
/// 使えないときのフォールバック）。
#[allow(clippy::too_many_arguments)]
fn composite_blur_via_temp(
    pixmap: &mut Pixmap,
    mask: Option<&Mask>,
    transform: Transform,
    bx0: f32,
    by0: f32,
    bx1: f32,
    by1: f32,
    cx: f32,
    cy: f32,
    color: [f32; 4],
    occluder: Option<ShadowOccluder>,
    coverage: impl Fn(f32, f32) -> f32,
) {
    let [cr, cg, cb, ca] = color;
    let bw = (bx1 - bx0) as i64;
    let bh = (by1 - by0) as i64;
    if bw <= 0 || bh <= 0 {
        return;
    }
    let bw = bw as u32;
    let bh = bh as u32;
    // 過大な bbox はガード（極端な σ でメモリが爆発しないよう）。
    if (bw as u64) * (bh as u64) > 64 * 1024 * 1024 {
        return;
    }
    let mut buf = vec![0u8; (bw as usize) * (bh as usize) * 4];
    for py in 0..bh {
        let scene_y = by0 + py as f32 + 0.5;
        let ly = scene_y - cy;
        for px in 0..bw {
            let scene_x = bx0 + px as f32 + 0.5;
            let lx = scene_x - cx;
            // 不透明 owner に覆われる内側は描かない（issue #659）。
            if let Some(occ) = occluder {
                if occ.contains(scene_x, scene_y) {
                    continue;
                }
            }
            let cov = coverage(lx, ly);
            if cov <= 0.0 {
                continue;
            }
            let a = ca * cov;
            let idx = ((py * bw + px) * 4) as usize;
            buf[idx] = (cr * a * 255.0 + 0.5) as u8;
            buf[idx + 1] = (cg * a * 255.0 + 0.5) as u8;
            buf[idx + 2] = (cb * a * 255.0 + 0.5) as u8;
            buf[idx + 3] = (a * 255.0 + 0.5) as u8;
        }
    }
    let Some(src) = PixmapRef::from_bytes(&buf, bw, bh) else {
        return;
    };
    let blit = transform.pre_translate(bx0, by0);
    pixmap.draw_pixmap(0, 0, src, &PixmapPaint::default(), blit, mask);
}

fn color_to_paint(color: [f32; 4]) -> Paint<'static> {
    let [r, g, b, a] = color;
    let mut paint = Paint::default();
    paint.set_color(
        Color::from_rgba(
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        )
        .unwrap_or(Color::TRANSPARENT),
    );
    paint.anti_alias = true;
    paint
}

fn push_rounded_rect(pb: &mut PathBuilder, x: f32, y: f32, w: f32, h: f32, r: f32) {
    let r = r.min(w / 2.0).min(h / 2.0);
    let kappa = 0.5522848;
    let k = r * kappa;

    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
}

fn rounded_rect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let mut pb = PathBuilder::new();
    push_rounded_rect(&mut pb, x, y, w, h, r);
    pb.finish()
}

fn draw_text_run(
    pixmap: &mut Pixmap,
    run_x: f32,
    run_y: f32,
    color: [f32; 4],
    data: &TextRunData,
    transform: Transform,
    mask: Option<&Mask>,
) {
    let paint = color_to_paint(color);
    let font_data = data.font.data.as_ref();
    let font = match FontRef::from_index(font_data, data.font.index) {
        Ok(f) => f,
        Err(_) => return,
    };
    let outlines = font.outline_glyphs();
    let font_size = data.font_size;
    let size = Size::new(font_size);
    let location = if data.normalized_coords.is_empty() {
        LocationRef::default()
    } else {
        LocationRef::new(normalized_coords_ref(&data.normalized_coords))
    };
    let skew = data.synthesis.skew_tangent;
    let embolden_width = data.synthesis.embolden;

    for glyph in &data.glyphs {
        // `.notdef` グリフはフォントがこのコードポイントを持たないことを意味する。
        // フォント任せの無音ボックスではなく意図的なプレースホルダ箱を描き、欠落が
        // 消えずに見えるようにする。
        if is_notdef(glyph) {
            draw_missing_glyph(pixmap, run_x, run_y, &paint, glyph, font_size, transform, mask);
            continue;
        }
        let outline = match outlines.get(GlyphId::new(glyph.id)) {
            Some(o) => o,
            None => continue,
        };

        let mut pen = TinySkiaPen {
            pb: PathBuilder::new(),
        };
        let settings = DrawSettings::unhinted(size, location);
        if outline.draw(settings, &mut pen).is_err() {
            continue;
        }
        let path = match pen.pb.finish() {
            Some(p) => p,
            None => continue,
        };

        let mut glyph_transform = transform
            .pre_translate(run_x + glyph.x, run_y + glyph.y)
            .pre_scale(1.0, -1.0);
        if let Some(tangent) = skew {
            glyph_transform = glyph_transform
                .pre_concat(Transform::from_row(1.0, 0.0, tangent, 1.0, 0.0, 0.0));
        }

        pixmap.fill_path(&path, &paint, FillRule::Winding, glyph_transform, mask);
        if let Some(stroke_width) = embolden_width {
            let stroke = Stroke {
                width: stroke_width,
                line_join: LineJoin::Round,
                line_cap: LineCap::Round,
                ..Stroke::default()
            };
            pixmap.stroke_path(&path, &paint, &stroke, glyph_transform, mask);
        }
    }

    for deco in &data.decorations {
        if let Some(rect) = tiny_skia::Rect::from_xywh(
            run_x + deco.x0,
            run_y + deco.y - deco.thickness * 0.5,
            (deco.x1 - deco.x0).max(0.0),
            deco.thickness,
        ) {
            let mut pb = PathBuilder::new();
            pb.push_rect(rect);
            if let Some(path) = pb.finish() {
                pixmap.fill_path(&path, &paint, FillRule::Winding, transform, mask);
            }
        }
    }
}

/// `.notdef` グリフ用の意図的なプレースホルダ箱を、テキスト色の中空ストローク矩形
/// として、ベースライン上の cap-height 帯に描く。ジオメトリは
/// `missing_glyph_placeholder` 経由で vello バックエンドと共有する。
fn draw_missing_glyph(
    pixmap: &mut Pixmap,
    run_x: f32,
    run_y: f32,
    paint: &Paint<'static>,
    glyph: &hayate_core::RenderGlyph,
    font_size: f32,
    transform: Transform,
    mask: Option<&Mask>,
) {
    let ph = missing_glyph_placeholder(glyph, font_size);
    if ph.width <= 0.0 || ph.height <= 0.0 {
        return;
    }
    let Some(rect) = tiny_skia::Rect::from_xywh(run_x + ph.x, run_y + ph.y, ph.width, ph.height)
    else {
        return;
    };
    let mut pb = PathBuilder::new();
    pb.push_rect(rect);
    let Some(path) = pb.finish() else {
        return;
    };
    let stroke = Stroke {
        width: ph.stroke_width,
        line_join: LineJoin::Miter,
        line_cap: LineCap::Butt,
        ..Stroke::default()
    };
    pixmap.stroke_path(&path, paint, &stroke, transform, mask);
}

fn draw_image(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    image: &RenderImage,
    transform: Transform,
    mask: Option<&Mask>,
) {
    if image.width == 0 || image.height == 0 {
        return;
    }

    let src_data = match image.alpha_type {
        RenderImageAlphaType::Premultiplied => image.data.data().to_vec(),
        _ => {
            let mut buf = image.data.data().to_vec();
            straight_to_premultiplied(&mut buf);
            buf
        }
    };

    let src_pixmap = match PixmapRef::from_bytes(&src_data, image.width, image.height) {
        Some(p) => p,
        None => return,
    };

    let sx = width / image.width as f32;
    let sy = height / image.height as f32;
    let img_transform = transform.pre_translate(x, y).pre_scale(sx, sy);

    pixmap.draw_pixmap(
        0,
        0,
        src_pixmap,
        &PixmapPaint::default(),
        img_transform,
        mask,
    );
}

struct TinySkiaPen {
    pb: PathBuilder,
}

impl OutlinePen for TinySkiaPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.pb.move_to(x, y);
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.pb.line_to(x, y);
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.pb.quad_to(cx0, cy0, x, y);
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.pb.cubic_to(cx0, cy0, cx1, cy1, x, y);
    }

    fn close(&mut self) {
        self.pb.close();
    }
}

#[cfg(test)]
mod shadow_tests {
    use super::*;

    /// 影ごとの形状定数を用意し、局所座標 `(lx, ly)` の被覆を返すヘルパ（`draw_blurred_rounded_rect`
    /// の per-pixel ループと同じ前計算）。
    fn coverage_at(width: f32, height: f32, radius: f32, std_dev: f32, lx: f32, ly: f32) -> f32 {
        let std_dev = std_dev.max(1e-5);
        let inv_std_dev = 1.0 / std_dev;
        let min_edge = width.min(height);
        let radius_max = 0.5 * min_edge;
        let radius = radius.max(0.0);
        let r0 = (radius * radius + (std_dev * SHADOW_BLUR_RADIUS_INNER_SIGMA).powi(2))
            .sqrt()
            .min(radius_max);
        let r1 = (radius * radius + (std_dev * SHADOW_BLUR_RADIUS_OUTER_SIGMA).powi(2))
            .sqrt()
            .min(radius_max);
        let exponent = 2.0 * r1 / r0;
        let inv_exponent = 1.0 / exponent;
        let delta = SHADOW_BLUR_ECCENTRICITY
            * std_dev
            * ((-(0.5 * inv_std_dev * width).powi(2)).exp()
                - (-(0.5 * inv_std_dev * height).powi(2)).exp());
        let adj_w = width + delta.min(0.0);
        let adj_h = height - delta.max(0.0);
        let scale = 0.5 * erf7(inv_std_dev * 0.5 * (adj_w.max(adj_h) - 0.5 * radius));
        blurred_rrect_coverage(
            lx, ly, adj_w, adj_h, min_edge, r1, exponent, inv_exponent, inv_std_dev, scale,
        )
    }

    #[test]
    fn coverage_is_high_inside_and_fades_to_zero_outside() {
        // 40x40 の矩形、角丸なし、σ=6。中心は実質不透明、外へ大きく離れると 0 へ。
        let center = coverage_at(40.0, 40.0, 0.0, 6.0, 0.0, 0.0);
        assert!(center > 0.9, "interior coverage should be near 1, got {center}");
        let far = coverage_at(40.0, 40.0, 0.0, 6.0, 60.0, 0.0);
        assert!(far < 0.02, "coverage far outside should vanish, got {far}");
    }

    #[test]
    fn coverage_decreases_monotonically_outward_from_the_edge() {
        // 縁（x=±20）から外向きに被覆は単調減少する（ガウス裾）。per-pixel で段差にならない。
        let mut prev = 1.1_f32;
        for step in 0..30 {
            let x = 20.0 + step as f32; // 縁から外へ 1px 刻み
            let cov = coverage_at(40.0, 40.0, 0.0, 6.0, x, 0.0);
            assert!(
                cov <= prev + 1e-4,
                "coverage must not increase outward at x={x}: {cov} > {prev}"
            );
            prev = cov;
        }
    }

    #[test]
    fn coverage_at_the_edge_is_about_half() {
        // ステップ関数 × ガウスなので、鋭い縁のちょうど上では被覆 ≈ 0.5。
        let edge = coverage_at(60.0, 60.0, 0.0, 8.0, 30.0, 0.0);
        assert!(
            (edge - 0.5).abs() < 0.1,
            "coverage at a sharp edge should be ~0.5, got {edge}"
        );
    }
}
