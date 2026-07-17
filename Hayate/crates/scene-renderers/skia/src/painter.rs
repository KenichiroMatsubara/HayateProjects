//! Skia キャンバスへ描く [`ScenePainter`] 実装（ADR-0054 crate 内部 seam・非公開）。
//!
//! Skia の `Canvas` は save/restore が matrix と clip を一体で積むスタックを持つため、
//! tiny-skia 実装のような手動の `transform_stack` / `clip_masks` は不要——
//! `push_transform`/`push_clip_rect`/`push_clip_draw_path` はすべて `canvas.save()`、
//! 対応する `pop_transform`/`pop_clip` はすべて `canvas.restore()` で対称に閉じる
//! （walk が push/pop を必ず対で呼ぶため、単一スタックでも取り違えない）。
//! 描画メソッドはローカル座標をそのまま Skia へ渡し、CTM の適用は Canvas に任せる。

use hayate_core::{
    build_draw_path, is_notdef, missing_glyph_placeholder, DrawFillRule, DrawLineCap, DrawLineJoin,
    PathSink, PathVerb, RenderGlyph, RenderImage, ScenePainter, StrokeStyle, TextRunData,
};
use skia_safe::{
    canvas::SrcRectConstraint,
    dash_path_effect,
    font_arguments::{variation_position::Coordinate, FontArguments, VariationPosition},
    paint::{Cap as PaintCap, Join as PaintJoin, Style as PaintStyle},
    Canvas, Color4f, Font, FontMgr, FourByteTag, Paint, Path, PathBuilder as SkPathBuilder,
    PathFillType, Point, RRect, Rect, SamplingOptions,
};
use skrifa::{
    raw::{tables::avar::SegmentMaps, FontRef, TableProvider},
    MetadataProvider,
};

use crate::resource_cache::PaintResourceCache;

pub struct SkiaPainter<'a> {
    canvas: &'a Canvas,
    resources: &'a mut PaintResourceCache,
}

impl<'a> SkiaPainter<'a> {
    pub fn new(canvas: &'a Canvas, resources: &'a mut PaintResourceCache) -> Self {
        Self { canvas, resources }
    }
}

fn paint_for(color: [f32; 4]) -> Paint {
    let [r, g, b, a] = color;
    let mut paint = Paint::new(Color4f::new(r, g, b, a), None);
    paint.set_anti_alias(true);
    paint
}

fn rrect_uniform(x: f32, y: f32, width: f32, height: f32, radius: f32) -> RRect {
    let r = radius.max(0.0);
    RRect::new_rect_xy(Rect::from_xywh(x, y, width, height), r, r)
}

impl ScenePainter for SkiaPainter<'_> {
    fn fill_rect(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        color: [f32; 4],
        corner_radius: f32,
    ) {
        let paint = paint_for(color);
        if corner_radius <= 0.0 {
            self.canvas
                .draw_rect(Rect::from_xywh(x, y, width, height), &paint);
        } else {
            self.canvas
                .draw_rrect(rrect_uniform(x, y, width, height, corner_radius), &paint);
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
        let bw = border_width.max(0.0);
        let inner_w = (width - 2.0 * bw).max(0.0);
        let inner_h = (height - 2.0 * bw).max(0.0);
        if inner_w <= 0.0 || inner_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }
        let inner_r = (outer_radius - bw).max(0.0);
        let mut pb = SkPathBuilder::new_with_fill_type(PathFillType::EvenOdd);
        pb.add_rrect(rrect_uniform(x, y, width, height, outer_radius), None, None);
        pb.add_rrect(
            rrect_uniform(x + bw, y + bw, inner_w, inner_h, inner_r),
            None,
            None,
        );
        let path = pb.detach();
        self.canvas.draw_path(&path, &paint_for(color));
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
        if inset_w <= 0.0 || inset_h <= 0.0 {
            self.fill_rect(x, y, width, height, color, outer_radius);
            return;
        }
        let inner_r = (outer_radius - half).max(0.0);
        let rrect = rrect_uniform(x + half, y + half, inset_w, inset_h, inner_r);
        let mut paint = paint_for(color);
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(bw);
        paint.set_stroke_cap(PaintCap::Butt);
        paint.set_stroke_join(PaintJoin::Miter);
        let dash = bw * 2.0;
        if let Some(effect) = dash_path_effect::new(&[dash, dash], 0.0) {
            paint.set_path_effect(effect);
        }
        self.canvas.draw_rrect(rrect, &paint);
    }

    fn fill_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        fill_rule: DrawFillRule,
        color: [f32; 4],
    ) {
        let Some(path) = verbs_to_path(verbs, fill_rule) else {
            return;
        };
        self.canvas.save();
        self.canvas.translate((x, y));
        self.canvas.draw_path(&path, &paint_for(color));
        self.canvas.restore();
    }

    fn stroke_path(
        &mut self,
        x: f32,
        y: f32,
        verbs: &[PathVerb],
        stroke: &StrokeStyle,
        color: [f32; 4],
    ) {
        if stroke.width <= 0.0 {
            return;
        }
        let Some(path) = verbs_to_path(verbs, DrawFillRule::NonZero) else {
            return;
        };
        let mut paint = paint_for(color);
        paint.set_style(PaintStyle::Stroke);
        paint.set_stroke_width(stroke.width);
        paint.set_stroke_miter(stroke.miter_limit);
        paint.set_stroke_cap(match stroke.cap {
            DrawLineCap::Butt => PaintCap::Butt,
            DrawLineCap::Round => PaintCap::Round,
            DrawLineCap::Square => PaintCap::Square,
        });
        paint.set_stroke_join(match stroke.join {
            DrawLineJoin::Miter => PaintJoin::Miter,
            DrawLineJoin::Round => PaintJoin::Round,
            DrawLineJoin::Bevel => PaintJoin::Bevel,
        });
        if !stroke.dash.is_empty() {
            if let Some(effect) = dash_path_effect::new(&stroke.dash, stroke.dash_offset) {
                paint.set_path_effect(effect);
            }
        }
        self.canvas.save();
        self.canvas.translate((x, y));
        self.canvas.draw_path(&path, &paint);
        self.canvas.restore();
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        draw_text_run(self.canvas, self.resources, x, y, color, data);
    }

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage) {
        draw_image(self.canvas, self.resources, x, y, width, height, data);
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        let [a, b, c, d, e, f] = transform;
        self.canvas.save();
        let matrix = skia_safe::Matrix::new_all(
            a as f32, c as f32, e as f32, b as f32, d as f32, f as f32, 0.0, 0.0, 1.0,
        );
        self.canvas.concat(&matrix);
    }

    fn pop_transform(&mut self) {
        self.canvas.restore();
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        self.canvas.save();
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        if radius > 0.0 {
            self.canvas
                .clip_rrect(rrect_uniform(x, y, width, height, radius), None, Some(true));
        } else {
            self.canvas
                .clip_rect(Rect::from_xywh(x, y, width, height), None, Some(true));
        }
    }

    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]) {
        self.canvas.save();
        if let Some(path) = verbs_to_path(verbs, DrawFillRule::NonZero) {
            self.canvas.clip_path(&path, None, Some(true));
        } else {
            // 退化クリップ（空パス）は何も通さない。walk のクリップ計数は save() で
            // すでに一致しているので、空矩形クリップで無 op を保証する。
            self.canvas
                .clip_rect(Rect::from_xywh(0.0, 0.0, 0.0, 0.0), None, None);
        }
    }

    fn pop_clip(&mut self) {
        self.canvas.restore();
    }
}

fn verbs_to_path(verbs: &[PathVerb], fill_rule: DrawFillRule) -> Option<Path> {
    let fill_type = match fill_rule {
        DrawFillRule::NonZero => PathFillType::Winding,
        DrawFillRule::EvenOdd => PathFillType::EvenOdd,
    };
    let mut sink = SkiaPathSink {
        pb: SkPathBuilder::new_with_fill_type(fill_type),
        has_points: false,
    };
    build_draw_path(verbs, &mut sink);
    if !sink.has_points {
        return None;
    }
    Some(sink.pb.detach())
}

struct SkiaPathSink {
    pb: SkPathBuilder,
    has_points: bool,
}

impl PathSink for SkiaPathSink {
    fn move_to(&mut self, x: f32, y: f32) {
        self.has_points = true;
        self.pb.move_to(Point::new(x, y));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.has_points = true;
        self.pb.line_to(Point::new(x, y));
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.has_points = true;
        self.pb.quad_to(Point::new(cx, cy), Point::new(x, y));
    }
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.has_points = true;
        self.pb
            .cubic_to(Point::new(c1x, c1y), Point::new(c2x, c2y), Point::new(x, y));
    }
    fn close(&mut self) {
        self.pb.close();
    }
}

thread_local! {
    /// スレッドごとに 1 度だけ構築する SkFontMgr。`FontMgr::default()`（`FontMgr::new` 相当）は
    /// 呼ぶたびに新しい SkFontMgr を生成し、Android では system font config XML のパース＋全
    /// システムフォントの列挙が走る。TextRun ごとに毎フレーム生成していたところ、実機
    /// （issue #803 の on-device 検証・OPPO A101OP）でフレームごとの `[SkFontMgr Android
    /// Parser]` ログ洪水とネイティブメモリ膨張→LMK kill（テキストの多いシーンで起動 15 秒後に
    /// プロセス死）を起こした。raster / GL の両 surface に共通の painter 品質問題であり、GL 対応
    /// （surface 非依存契約 ADR-0146 §3）とは独立の修正。
    static FONT_MGR: FontMgr = FontMgr::default();
}

/// スレッド共有の SkFontMgr で `f` を実行する（生成は重いので使い回す）。
fn shared_font_mgr<R>(f: impl FnOnce(&FontMgr) -> R) -> R {
    FONT_MGR.with(f)
}

thread_local! {
    /// `Blob::id()`＋face index＋正規化 variation 座標をキーにした typeface の常駐キャッシュ。
    /// `new_from_data` はフォントバイト列全体を SkData へコピーするため、TextRun ごと・
    /// フレームごとの再構築は実機でネイティブメモリ膨張→LMK kill を起こした（issue #803 の
    /// on-device 検証）。core の画像アトラス（`RenderImage`）と同じ「Blob が生きている間は
    /// id が安定」という前提でキーにする。variation 座標をキーに含めるのは、variable font
    /// の各インスタンス（wght 等）が SkTypeface 単位で焼き込まれるため。組み合わせ数は
    /// 「フォント数 × 使用ウェイト数」でたかだか数十なので無制限で保持する。
    static TYPEFACE_CACHE: std::cell::RefCell<
        std::collections::HashMap<(u64, u32, Vec<i16>), Option<skia_safe::Typeface>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// avar の区分線形写像（pre-avar 正規化 → post-avar 正規化）を逆向きに評価する。
/// 区分は from/to とも単調非減少なので、post 側の区間を見つけて線形補間で from 側へ戻す。
fn inverse_avar_segment(map: &SegmentMaps, post: f32) -> f32 {
    let maps = map.axis_value_maps();
    let from = |i: usize| maps[i].from_coordinate().to_f32();
    let to = |i: usize| maps[i].to_coordinate().to_f32();
    match maps.len() {
        0 => return post,
        1 => return post - to(0) + from(0),
        _ => {}
    }
    if post <= to(0) {
        return from(0);
    }
    for i in 1..maps.len() {
        if post <= to(i) {
            let span = to(i) - to(i - 1);
            if span <= 0.0 {
                return from(i);
            }
            return from(i - 1) + (post - to(i - 1)) * (from(i) - from(i - 1)) / span;
        }
    }
    from(maps.len() - 1)
}

/// Parley 由来の正規化 variation 座標（avar 適用後・F2Dot14・fvar 軸順）を fvar の
/// design 座標へ戻す。Skia の `FontArguments` は design 座標しか受けないため、avar が
/// あれば区分線形写像を逆向きに評価してから、fvar の min/default/max で線形展開する。
fn design_coords_from_normalized(
    font: &hayate_core::RenderFont,
    normalized: &[i16],
) -> Vec<Coordinate> {
    let bytes: &[u8] = font.data.as_ref();
    let Ok(font_ref) = FontRef::from_index(bytes, font.index) else {
        return Vec::new();
    };
    let axes = font_ref.axes();
    let avar = font_ref.avar().ok();
    let mut out = Vec::with_capacity(axes.len());
    for (i, axis) in axes.iter().enumerate() {
        let Some(&raw) = normalized.get(i) else {
            break;
        };
        let post = f32::from(raw) / 16384.0;
        let pre = match avar
            .as_ref()
            .and_then(|a| a.axis_segment_maps().iter().nth(i))
        {
            Some(Ok(map)) => inverse_avar_segment(&map, post),
            _ => post,
        };
        let (min, def, max) = (axis.min_value(), axis.default_value(), axis.max_value());
        let design = if pre >= 0.0 {
            def + pre * (max - def)
        } else {
            def + pre * (def - min)
        };
        let tag = FourByteTag::from(u32::from_be_bytes(axis.tag().to_be_bytes()));
        out.push(Coordinate {
            axis: tag,
            value: design,
        });
    }
    out
}

/// `RenderFont` のバイト列から、`normalized_coords` の variable font インスタンスを
/// 焼き込んだ SkTypeface を作る（`Blob::id()`＋座標キーの常駐キャッシュ越し）。
/// 座標を無視すると variable font は fvar 既定インスタンスで描かれる——バンドルの
/// NotoSansJP は既定が wght=100（Thin）なので、全テキストがヘアラインになり UI 全体が
/// 「淡く」見える実回帰があった（vello/tiny-skia は座標を消費する。共有
/// css_pixels の font-weight ケースがこの契約を固定する）。
fn cached_typeface(
    font: &hayate_core::RenderFont,
    normalized_coords: &[i16],
) -> Option<skia_safe::Typeface> {
    let key = (font.data.id(), font.index, normalized_coords.to_vec());
    TYPEFACE_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| {
                let bytes: &[u8] = font.data.as_ref();
                let base = shared_font_mgr(|mgr| mgr.new_from_data(bytes, font.index as usize))?;
                if normalized_coords.is_empty() {
                    return Some(base);
                }
                let coords = design_coords_from_normalized(font, normalized_coords);
                if coords.is_empty() {
                    return Some(base);
                }
                let args = FontArguments::new().set_variation_design_position(VariationPosition {
                    coordinates: &coords,
                });
                // clone 失敗（非 variable font 等）は既定インスタンスへフォールバック。
                base.clone_with_arguments(&args).or(Some(base))
            })
            .clone()
    })
}

fn typeface_for(data: &TextRunData) -> Option<skia_safe::Typeface> {
    cached_typeface(&data.font, &data.normalized_coords)
}

fn draw_text_run(
    canvas: &Canvas,
    resources: &mut PaintResourceCache,
    run_x: f32,
    run_y: f32,
    color: [f32; 4],
    data: &TextRunData,
) {
    let Some(typeface) = typeface_for(data) else {
        return;
    };
    let mut font = Font::new(typeface, data.font_size);
    font.set_subpixel(true);
    font.set_hinting(skia_safe::FontHinting::None);
    // ビットマップ絵文字（CBDT/CBLC・sbix）が描かれるために必須（既定 false）。COLR/CPAL
    // ベクタカラーグリフはこのフラグと無関係に自動判定される（ADR-0146 §4）。
    font.set_embedded_bitmaps(true);
    if let Some(tangent) = data.synthesis.skew_tangent {
        font.set_skew_x(tangent);
    }
    if data.synthesis.embolden.is_some() {
        font.set_embolden(true);
    }

    let paint = paint_for(color);

    // notdef グリフはフォールバックの無音アウトラインではなく、意図的なプレースホルダ
    // 箱を描く（vello / tiny-skia と同じ geometry を共有 `missing_glyph_placeholder` から借りる）。
    for glyph in &data.glyphs {
        if is_notdef(glyph) {
            draw_missing_glyph(canvas, run_x, run_y, &paint, glyph, data.font_size);
        }
    }

    if let Some(blob) = resources.text_blob_for(data, &font) {
        canvas.draw_text_blob(&blob, (run_x, run_y), &paint);
    }

    for deco in &data.decorations {
        let rect = Rect::from_xywh(
            run_x + deco.x0,
            run_y + deco.y - deco.thickness * 0.5,
            (deco.x1 - deco.x0).max(0.0),
            deco.thickness,
        );
        canvas.draw_rect(rect, &paint);
    }
}

fn draw_missing_glyph(
    canvas: &Canvas,
    run_x: f32,
    run_y: f32,
    paint: &Paint,
    glyph: &RenderGlyph,
    font_size: f32,
) {
    let ph = missing_glyph_placeholder(glyph, font_size);
    if ph.width <= 0.0 || ph.height <= 0.0 {
        return;
    }
    let mut stroke = paint.clone();
    stroke.set_style(PaintStyle::Stroke);
    stroke.set_stroke_width(ph.stroke_width);
    stroke.set_stroke_cap(PaintCap::Butt);
    stroke.set_stroke_join(PaintJoin::Miter);
    canvas.draw_rect(
        Rect::from_xywh(run_x + ph.x, run_y + ph.y, ph.width, ph.height),
        &stroke,
    );
}

fn draw_image(
    canvas: &Canvas,
    resources: &mut PaintResourceCache,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    image: &RenderImage,
) {
    let Some(sk_image) = resources.image_for(image) else {
        return;
    };
    let dst = Rect::from_xywh(x, y, width, height);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    canvas.draw_image_rect_with_sampling_options(
        &sk_image,
        None::<(&Rect, SrcRectConstraint)>,
        dst,
        SamplingOptions::default(),
        &paint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RCHandle の native ポインタ（同一 SkFontMgr インスタンスかの同定用）。
    /// SAFETY: `RCHandle<N>` は `NonNull<N>` の透明ラッパ（skia-safe は `=0.99.0` に厳密ピン）。
    fn native_ptr(mgr: &FontMgr) -> *const std::ffi::c_void {
        unsafe { std::mem::transmute_copy::<FontMgr, *const std::ffi::c_void>(mgr) }
    }

    #[test]
    fn typefaces_are_cached_per_font_blob_and_reused_across_frames() {
        // `FontMgr::new_from_data` はフォントバイト列全体を SkData へコピーする。TextRun ごと・
        // フレームごとに typeface を作り直すと、実機（issue #803 検証・OPPO A101OP）でネイティブ
        // メモリが伸び続け起動 ~70 秒で LMK kill された。`Blob::id()`（同一フォントが生きて
        // いる間は安定、core の画像アトラスと同じキー設計）で typeface を使い回すことを固定する。
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/assets/twemoji_smiley_sbix.ttf"
        ))
        .expect("test font asset");
        let font =
            hayate_core::RenderFont::new(hayate_core::Blob::new(std::sync::Arc::new(bytes)), 0);
        let a = cached_typeface(&font, &[]).expect("typeface from valid font bytes");
        let b = cached_typeface(&font, &[]).expect("typeface from valid font bytes");
        assert_eq!(
            unsafe { std::mem::transmute_copy::<skia_safe::Typeface, *const std::ffi::c_void>(&a) },
            unsafe { std::mem::transmute_copy::<skia_safe::Typeface, *const std::ffi::c_void>(&b) },
            "the same RenderFont (same Blob id + index) must reuse the cached SkTypeface"
        );
    }

    #[test]
    fn the_font_mgr_is_constructed_once_per_thread_and_reused() {
        // SkFontMgr の生成は重い（Android では system font config XML のパース＋全システム
        // フォントの列挙）。TextRun ごとに毎フレーム生成すると、実機（issue #803 の検証）で
        // フレームごとの XML パースとネイティブメモリ膨張→LMK kill を起こした。同一スレッド
        // 内では同じインスタンスが返ることを固定する（clone を両方生かしたまま native
        // ポインタを比較——毎回生成する誤実装ならこの 2 つは別インスタンスになる）。
        let a = shared_font_mgr(|mgr| mgr.clone());
        let b = shared_font_mgr(|mgr| mgr.clone());
        assert_eq!(
            native_ptr(&a),
            native_ptr(&b),
            "shared_font_mgr must hand out the same per-thread SkFontMgr instance"
        );
    }
}
