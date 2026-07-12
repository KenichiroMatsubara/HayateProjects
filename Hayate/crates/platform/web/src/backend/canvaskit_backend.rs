//! Skia CanvasKit backend（ADR-0148 / Phase 2-3・**DRAFT / 未検証**）。
//!
//! CanvasKit（Google Skia の公式 wasm ビルド）を wasm-bindgen 経由で駆動する web backend。
//! skia-safe（ネイティブ、ADR-0146）の web 対応版で、同じ Skia の描画結果に収束させる。
//! シーン歩行（`render_scene_graph`）と [`ScenePainter`] 実装はこのファイルに閉じ、
//! CanvasKit との JS 境界は [`glue`] モジュール（`js/canvaskit_glue.js`）だけ（ADR-0072 維持）。
//!
//! painter は `crates/scene-renderers/skia/src/painter.rs`（ネイティブ skia_safe）の移植。
//! CanvasKit の `Canvas`/`Paint`/`Path`/`Font` は skia_safe とほぼ 1:1 なので、描画ロジックは
//! そのまま写し、型だけ extern 束縛に置き換えている。
//!
//! **この環境（web セッション）では wasm32 target / wasm-bindgen が無くビルド・検証できない。**
//! wasm ツールチェーンの整った環境でのビルドと Playwright での視覚検証を前提とする。
//! 未確定箇所は `TODO(ADR-0148 ...)` で明示している（variable font 座標・per-layer present・
//! CanvasKit オブジェクトの delete 規律・GL/SW surface 選択・resize）。

use std::cell::RefCell;
use std::collections::HashMap;

use hayate_core::{
    build_draw_path, is_notdef, missing_glyph_placeholder, render_scene_graph, DrawFillRule,
    DrawLineCap, DrawLineJoin, PathSink, PathVerb, RenderGlyph, RenderImage, RenderImageAlphaType,
    SceneGraph, ScenePainter, StrokeStyle, TextRunData,
};
use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use super::{js_to_anyhow, CanvasBackend, ClearColor, SceneRendererKind};

// ── CanvasKit への JS 境界（js/canvaskit_glue.js への extern 束縛） ─────────────────
//
// 生成系（`new CanvasKit.Paint()` 等）と enum 値は glue の関数越しに取り、描画メソッドは
// 下の extern type のメソッドとして CanvasKit オブジェクトへ直接呼ぶ。
#[wasm_bindgen(module = "/js/canvaskit_glue.js")]
extern "C" {
    #[wasm_bindgen(js_name = loadCanvasKit, catch)]
    async fn load_canvaskit(locate_base: &str) -> Result<JsValue, JsValue>;

    #[wasm_bindgen(js_name = makeSurface)]
    fn make_surface(ck: &CanvasKit, canvas: &HtmlCanvasElement) -> Option<CkSurface>;

    #[wasm_bindgen(js_name = newPaint)]
    fn new_paint(ck: &CanvasKit) -> CkPaint;
    #[wasm_bindgen(js_name = newPath)]
    fn new_path(ck: &CanvasKit) -> CkPath;
    #[wasm_bindgen(js_name = newFont)]
    fn new_font(ck: &CanvasKit, typeface: &CkTypeface, size: f32) -> CkFont;
    #[wasm_bindgen(js_name = color4f)]
    fn color4f(ck: &CanvasKit, r: f32, g: f32, b: f32, a: f32) -> JsValue;
    #[wasm_bindgen(js_name = xywhRect)]
    fn xywh_rect(ck: &CanvasKit, x: f32, y: f32, w: f32, h: f32) -> JsValue;
    #[wasm_bindgen(js_name = rrectXY)]
    fn rrect_xy(ck: &CanvasKit, x: f32, y: f32, w: f32, h: f32, radius: f32) -> JsValue;
    #[wasm_bindgen(js_name = makeDashPathEffect)]
    fn make_dash_path_effect(ck: &CanvasKit, on: f32, off: f32, phase: f32) -> Option<CkPathEffect>;
    #[wasm_bindgen(js_name = makeTypefaceFromData)]
    fn make_typeface_from_data(ck: &CanvasKit, bytes: &[u8]) -> Option<CkTypeface>;
    #[wasm_bindgen(js_name = makeImage)]
    fn make_image(ck: &CanvasKit, bytes: &[u8], width: i32, height: i32, alpha_type: &JsValue)
        -> Option<CkImage>;

    #[wasm_bindgen(js_name = paintStyleFill)]
    fn paint_style_fill(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = paintStyleStroke)]
    fn paint_style_stroke(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = strokeCapButt)]
    fn stroke_cap_butt(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = strokeCapRound)]
    fn stroke_cap_round(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = strokeCapSquare)]
    fn stroke_cap_square(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = strokeJoinMiter)]
    fn stroke_join_miter(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = strokeJoinRound)]
    fn stroke_join_round(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = strokeJoinBevel)]
    fn stroke_join_bevel(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = fillTypeWinding)]
    fn fill_type_winding(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = fillTypeEvenOdd)]
    fn fill_type_even_odd(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = alphaTypeOpaque)]
    fn alpha_type_opaque(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = alphaTypePremul)]
    fn alpha_type_premul(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = alphaTypeUnpremul)]
    fn alpha_type_unpremul(ck: &CanvasKit) -> JsValue;
    #[wasm_bindgen(js_name = clipOpIntersect)]
    fn clip_op_intersect(ck: &CanvasKit) -> JsValue;
}

#[wasm_bindgen]
extern "C" {
    type CanvasKit;

    type CkSurface;
    #[wasm_bindgen(method, js_name = getCanvas)]
    fn get_canvas(this: &CkSurface) -> CkCanvas;
    #[wasm_bindgen(method)]
    fn flush(this: &CkSurface);

    type CkCanvas;
    #[wasm_bindgen(method)]
    fn save(this: &CkCanvas);
    #[wasm_bindgen(method)]
    fn restore(this: &CkCanvas);
    #[wasm_bindgen(method)]
    fn translate(this: &CkCanvas, dx: f32, dy: f32);
    #[wasm_bindgen(method)]
    fn scale(this: &CkCanvas, sx: f32, sy: f32);
    #[wasm_bindgen(method)]
    fn concat(this: &CkCanvas, matrix: Vec<f64>);
    #[wasm_bindgen(method)]
    fn clear(this: &CkCanvas, color: &JsValue);
    #[wasm_bindgen(method, js_name = drawRect)]
    fn draw_rect(this: &CkCanvas, rect: &JsValue, paint: &CkPaint);
    #[wasm_bindgen(method, js_name = drawRRect)]
    fn draw_rrect(this: &CkCanvas, rrect: &JsValue, paint: &CkPaint);
    #[wasm_bindgen(method, js_name = drawPath)]
    fn draw_path(this: &CkCanvas, path: &CkPath, paint: &CkPaint);
    #[wasm_bindgen(method, js_name = clipRect)]
    fn clip_rect(this: &CkCanvas, rect: &JsValue, op: &JsValue, aa: bool);
    #[wasm_bindgen(method, js_name = clipRRect)]
    fn clip_rrect(this: &CkCanvas, rrect: &JsValue, op: &JsValue, aa: bool);
    #[wasm_bindgen(method, js_name = clipPath)]
    fn clip_path(this: &CkCanvas, path: &CkPath, op: &JsValue, aa: bool);
    #[wasm_bindgen(method, js_name = drawImageRect)]
    fn draw_image_rect(
        this: &CkCanvas,
        image: &CkImage,
        src: &JsValue,
        dst: &JsValue,
        paint: &CkPaint,
        fast_sample: bool,
    );
    #[wasm_bindgen(method, js_name = drawGlyphs)]
    fn draw_glyphs(
        this: &CkCanvas,
        glyphs: Vec<u16>,
        positions: Vec<f32>,
        x: f32,
        y: f32,
        font: &CkFont,
        paint: &CkPaint,
    );

    type CkPaint;
    #[wasm_bindgen(method, js_name = setColor)]
    fn set_color(this: &CkPaint, color: &JsValue);
    #[wasm_bindgen(method, js_name = setStyle)]
    fn set_style(this: &CkPaint, style: &JsValue);
    #[wasm_bindgen(method, js_name = setStrokeWidth)]
    fn set_stroke_width(this: &CkPaint, w: f32);
    #[wasm_bindgen(method, js_name = setStrokeCap)]
    fn set_stroke_cap(this: &CkPaint, cap: &JsValue);
    #[wasm_bindgen(method, js_name = setStrokeJoin)]
    fn set_stroke_join(this: &CkPaint, join: &JsValue);
    #[wasm_bindgen(method, js_name = setStrokeMiter)]
    fn set_stroke_miter(this: &CkPaint, miter: f32);
    #[wasm_bindgen(method, js_name = setPathEffect)]
    fn set_path_effect(this: &CkPaint, effect: &CkPathEffect);
    #[wasm_bindgen(method)]
    fn delete(this: &CkPaint);

    type CkPath;
    #[wasm_bindgen(method, js_name = moveTo)]
    fn move_to(this: &CkPath, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = lineTo)]
    fn line_to(this: &CkPath, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = quadTo)]
    fn quad_to(this: &CkPath, cx: f32, cy: f32, x: f32, y: f32);
    #[wasm_bindgen(method, js_name = cubicTo)]
    fn cubic_to(this: &CkPath, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32);
    #[wasm_bindgen(method)]
    fn close(this: &CkPath);
    #[wasm_bindgen(method, js_name = setFillType)]
    fn set_fill_type(this: &CkPath, ft: &JsValue);
    #[wasm_bindgen(method, js_name = addRRect)]
    fn add_rrect(this: &CkPath, rrect: &JsValue);
    #[wasm_bindgen(method, js_name = delete)]
    fn delete_path(this: &CkPath);

    type CkFont;
    #[wasm_bindgen(method, js_name = setSubpixel)]
    fn set_subpixel(this: &CkFont, on: bool);
    #[wasm_bindgen(method, js_name = setEmbeddedBitmaps)]
    fn set_embedded_bitmaps(this: &CkFont, on: bool);
    #[wasm_bindgen(method, js_name = setSkewX)]
    fn set_skew_x(this: &CkFont, tangent: f32);
    #[wasm_bindgen(method, js_name = setEmbolden)]
    fn set_embolden(this: &CkFont, on: bool);
    #[wasm_bindgen(method, js_name = delete)]
    fn delete_font(this: &CkFont);

    type CkTypeface;
    type CkImage;
    type CkPathEffect;
}

// ── backend 本体 ──────────────────────────────────────────────────────────────────

pub(crate) struct SelectedBackend {
    ck: CanvasKit,
    surface: CkSurface,
    canvas_el: HtmlCanvasElement,
    content_scale: f32,
    // Blob id → CanvasKit Typeface のキャッシュ（ネイティブ skia の TYPEFACE_CACHE 相当）。
    // TextRun ごと・フレームごとの再パースを避ける。TODO(ADR-0148 Phase 4): variation 座標を
    // キーに含める／退去ポリシー。
    typefaces: RefCell<HashMap<u64, Option<CkTypeface>>>,
}

impl SelectedBackend {
    pub(crate) async fn init(canvas: HtmlCanvasElement) -> Result<Self, JsValue> {
        // TODO(ADR-0148 Phase 2): locate_base はアプリの配信構成から渡す（canvaskit.wasm の URL）。
        // 暫定でルート相対。deploy-pages に asset 配信を足すまでは 404 になりうる。
        let ck: CanvasKit = load_canvaskit("/")
            .await
            .map_err(|e| JsValue::from_str(&format!("CanvasKit init failed: {e:?}")))?
            .unchecked_into();
        let surface = make_surface(&ck, &canvas)
            .ok_or_else(|| JsValue::from_str("CanvasKit surface unavailable (WebGL/SW both failed)"))?;
        Ok(Self {
            ck,
            surface,
            canvas_el: canvas,
            content_scale: 1.0,
            typefaces: RefCell::new(HashMap::new()),
        })
    }
}

impl CanvasBackend for SelectedBackend {
    fn kind(&self) -> SceneRendererKind {
        SceneRendererKind::Canvaskit
    }

    fn render_scene(&mut self, scene: &SceneGraph, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        let canvas = self.surface.get_canvas();
        canvas.save();
        let [r, g, b, a] = clear_color;
        canvas.clear(&color4f(&self.ck, r, g, b, a));
        if self.content_scale != 1.0 {
            canvas.scale(self.content_scale, self.content_scale);
        }
        {
            let mut painter = CanvaskitPainter {
                ck: &self.ck,
                canvas: &canvas,
                typefaces: &self.typefaces,
            };
            render_scene_graph(scene, &mut painter);
        }
        canvas.restore();
        // GPU（WebGL）surface は flush で GL バッファへ確定する。SW でも安全な no-op 相当。
        self.surface.flush();
        Ok(())
    }

    fn clear(&mut self, clear_color: ClearColor) -> Result<(), anyhow::Error> {
        self.render_scene(&SceneGraph::new(), clear_color)
    }

    // TODO(ADR-0148 Phase 5): per-layer present（compositor）は未実装。v1 は毎フレーム全面
    // render_scene（tiny-skia 初期実装と同じ姿勢）。supports_layer_present=false で呼び出し側
    // （canvas.rs）が全面経路にフォールバックする。
    fn supports_layer_present(&self) -> bool {
        false
    }

    fn resize(&mut self, _width: u32, _height: u32, content_scale: f32) {
        self.content_scale = content_scale.max(1.0);
        // TODO(ADR-0148 Phase 2): CanvasKit surface の再生成が要る（GL surface は canvas の
        // backing store サイズに追従しない）。canvas_el のサイズ更新後に make_surface し直す。
        let _ = &self.canvas_el;
    }
}

// ── ScenePainter 実装（skia painter.rs の移植） ────────────────────────────────────

struct CanvaskitPainter<'a> {
    ck: &'a CanvasKit,
    canvas: &'a CkCanvas,
    typefaces: &'a RefCell<HashMap<u64, Option<CkTypeface>>>,
}

impl<'a> CanvaskitPainter<'a> {
    fn paint_for(&self, color: [f32; 4]) -> CkPaint {
        let [r, g, b, a] = color;
        let paint = new_paint(self.ck);
        paint.set_color(&color4f(self.ck, r, g, b, a));
        paint
    }
}

impl ScenePainter for CanvaskitPainter<'_> {
    fn fill_rect(&mut self, x: f32, y: f32, width: f32, height: f32, color: [f32; 4], corner_radius: f32) {
        let paint = self.paint_for(color);
        if corner_radius <= 0.0 {
            self.canvas.draw_rect(&xywh_rect(self.ck, x, y, width, height), &paint);
        } else {
            self.canvas
                .draw_rrect(&rrect_xy(self.ck, x, y, width, height, corner_radius.max(0.0)), &paint);
        }
        paint.delete();
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
        // even-odd の 2 RRect 差分（外形 − 内形）で角丸リングを塗る（skia painter と同じ）。
        let path = new_path(self.ck);
        path.set_fill_type(&fill_type_even_odd(self.ck));
        path.add_rrect(&rrect_xy(self.ck, x, y, width, height, outer_radius.max(0.0)));
        path.add_rrect(&rrect_xy(self.ck, x + bw, y + bw, inner_w, inner_h, inner_r));
        let paint = self.paint_for(color);
        self.canvas.draw_path(&path, &paint);
        paint.delete();
        path.delete_path();
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
        let paint = self.paint_for(color);
        paint.set_style(&paint_style_stroke(self.ck));
        paint.set_stroke_width(bw);
        paint.set_stroke_cap(&stroke_cap_butt(self.ck));
        paint.set_stroke_join(&stroke_join_miter(self.ck));
        let dash = bw * 2.0;
        if let Some(effect) = make_dash_path_effect(self.ck, dash, dash, 0.0) {
            paint.set_path_effect(&effect);
        }
        self.canvas
            .draw_rrect(&rrect_xy(self.ck, x + half, y + half, inset_w, inset_h, inner_r), &paint);
        paint.delete();
    }

    fn fill_path(&mut self, x: f32, y: f32, verbs: &[PathVerb], fill_rule: DrawFillRule, color: [f32; 4]) {
        let Some(path) = self.verbs_to_path(verbs, fill_rule) else {
            return;
        };
        let paint = self.paint_for(color);
        self.canvas.save();
        self.canvas.translate(x, y);
        self.canvas.draw_path(&path, &paint);
        self.canvas.restore();
        paint.delete();
        path.delete_path();
    }

    fn stroke_path(&mut self, x: f32, y: f32, verbs: &[PathVerb], stroke: &StrokeStyle, color: [f32; 4]) {
        if stroke.width <= 0.0 {
            return;
        }
        let Some(path) = self.verbs_to_path(verbs, DrawFillRule::NonZero) else {
            return;
        };
        let paint = self.paint_for(color);
        paint.set_style(&paint_style_stroke(self.ck));
        paint.set_stroke_width(stroke.width);
        paint.set_stroke_miter(stroke.miter_limit);
        paint.set_stroke_cap(&match stroke.cap {
            DrawLineCap::Butt => stroke_cap_butt(self.ck),
            DrawLineCap::Round => stroke_cap_round(self.ck),
            DrawLineCap::Square => stroke_cap_square(self.ck),
        });
        paint.set_stroke_join(&match stroke.join {
            DrawLineJoin::Miter => stroke_join_miter(self.ck),
            DrawLineJoin::Round => stroke_join_round(self.ck),
            DrawLineJoin::Bevel => stroke_join_bevel(self.ck),
        });
        // TODO(ADR-0148 Phase 3): 任意 dash 配列は CanvasKit.PathEffect.MakeDash([..], phase)。
        // glue の makeDashPathEffect は 2 要素固定なので、可変長 dash 用の口を足す。
        if !stroke.dash.is_empty() && stroke.dash.len() >= 2 {
            if let Some(effect) = make_dash_path_effect(self.ck, stroke.dash[0], stroke.dash[1], stroke.dash_offset) {
                paint.set_path_effect(&effect);
            }
        }
        self.canvas.save();
        self.canvas.translate(x, y);
        self.canvas.draw_path(&path, &paint);
        self.canvas.restore();
        paint.delete();
        path.delete_path();
    }

    fn draw_text_run(&mut self, x: f32, y: f32, color: [f32; 4], data: &TextRunData) {
        self.draw_text_run_impl(x, y, color, data);
    }

    fn draw_image(&mut self, x: f32, y: f32, width: f32, height: f32, data: &RenderImage) {
        if data.width == 0 || data.height == 0 {
            return;
        }
        let alpha = match data.alpha_type {
            RenderImageAlphaType::Opaque => alpha_type_opaque(self.ck),
            RenderImageAlphaType::Alpha => alpha_type_unpremul(self.ck),
            RenderImageAlphaType::Premultiplied => alpha_type_premul(self.ck),
        };
        let Some(image) = make_image(self.ck, data.data.data(), data.width as i32, data.height as i32, &alpha)
        else {
            return;
        };
        let src = xywh_rect(self.ck, 0.0, 0.0, data.width as f32, data.height as f32);
        let dst = xywh_rect(self.ck, x, y, width, height);
        let paint = new_paint(self.ck);
        // fast_sample=false で線形サンプリング（skia painter の SamplingOptions::default 相当）。
        self.canvas.draw_image_rect(&image, &src, &dst, &paint, false);
        paint.delete();
    }

    fn push_transform(&mut self, transform: [f64; 6]) {
        let [a, b, c, d, e, f] = transform;
        self.canvas.save();
        // 3x3 行優先 [a c e; b d f; 0 0 1]（skia painter の Matrix::new_all と同じ並び）。
        self.canvas.concat(vec![a, c, e, b, d, f, 0.0, 0.0, 1.0]);
    }

    fn pop_transform(&mut self) {
        self.canvas.restore();
    }

    fn push_clip_rect(&mut self, x: f32, y: f32, width: f32, height: f32, corner_radii: [f32; 4]) {
        self.canvas.save();
        let radius = corner_radii.iter().copied().fold(0.0_f32, f32::max);
        let op = clip_op_intersect(self.ck);
        if radius > 0.0 {
            self.canvas
                .clip_rrect(&rrect_xy(self.ck, x, y, width, height, radius), &op, true);
        } else {
            self.canvas.clip_rect(&xywh_rect(self.ck, x, y, width, height), &op, true);
        }
    }

    fn push_clip_draw_path(&mut self, verbs: &[PathVerb]) {
        self.canvas.save();
        let op = clip_op_intersect(self.ck);
        if let Some(path) = self.verbs_to_path(verbs, DrawFillRule::NonZero) {
            self.canvas.clip_path(&path, &op, true);
            path.delete_path();
        } else {
            // 退化クリップ（空パス）は何も通さない空矩形クリップ。walk のクリップ計数は
            // save() で一致済み。
            self.canvas.clip_rect(&xywh_rect(self.ck, 0.0, 0.0, 0.0, 0.0), &op, false);
        }
    }

    fn pop_clip(&mut self) {
        self.canvas.restore();
    }
}

impl CanvaskitPainter<'_> {
    fn verbs_to_path(&self, verbs: &[PathVerb], fill_rule: DrawFillRule) -> Option<CkPath> {
        let path = new_path(self.ck);
        path.set_fill_type(&match fill_rule {
            DrawFillRule::NonZero => fill_type_winding(self.ck),
            DrawFillRule::EvenOdd => fill_type_even_odd(self.ck),
        });
        let mut sink = CkPathSink { path: &path, has_points: false };
        build_draw_path(verbs, &mut sink);
        if !sink.has_points {
            path.delete_path();
            return None;
        }
        Some(path)
    }

    fn typeface_for(&self, data: &TextRunData) -> Option<CkTypeface> {
        // TODO(ADR-0148 Phase 4): variable font の normalized_coords をキー＆焼き込みに含める
        // （ネイティブ skia painter の cached_typeface / design_coords_from_normalized 相当）。
        // 現状は既定インスタンスのみ — variable font は fvar 既定（NotoSansJP は Thin）で描かれ、
        // 全テキストがヘアライン化する既知の未対応。CkTypeface は Clone を持たないため、
        // ここでは毎回 make し直す（キャッシュ HashMap は将来の owner 保持用の骨組み）。
        let _ = &self.typefaces;
        make_typeface_from_data(self.ck, data.font.data.as_ref())
    }

    fn draw_text_run_impl(&self, run_x: f32, run_y: f32, color: [f32; 4], data: &TextRunData) {
        let Some(typeface) = self.typeface_for(data) else {
            return;
        };
        let font = new_font(self.ck, &typeface, data.font_size);
        font.set_subpixel(true);
        // ビットマップ絵文字（CBDT/CBLC・sbix）に必須。COLR/CPAL は自動判定（ADR-0146 §4）。
        font.set_embedded_bitmaps(true);
        if let Some(tangent) = data.synthesis.skew_tangent {
            font.set_skew_x(tangent);
        }
        if data.synthesis.embolden.is_some() {
            font.set_embolden(true);
        }

        let paint = self.paint_for(color);

        // notdef は無音アウトラインでなく意図的なプレースホルダ箱（vello/tiny-skia/skia と共有）。
        let mut glyph_ids: Vec<u16> = Vec::with_capacity(data.glyphs.len());
        let mut positions: Vec<f32> = Vec::with_capacity(data.glyphs.len() * 2);
        for glyph in &data.glyphs {
            if is_notdef(glyph) {
                self.draw_missing_glyph(run_x, run_y, &paint, glyph, data.font_size);
            } else {
                glyph_ids.push(glyph.id as u16);
                positions.push(glyph.x);
                positions.push(glyph.y);
            }
        }
        if !glyph_ids.is_empty() {
            // CanvasKit: drawGlyphs(glyphs, positions[x0,y0,...], x, y, font, paint)。
            self.canvas
                .draw_glyphs(glyph_ids, positions, run_x, run_y, &font, &paint);
        }

        for deco in &data.decorations {
            self.canvas.draw_rect(
                &xywh_rect(
                    self.ck,
                    run_x + deco.x0,
                    run_y + deco.y - deco.thickness * 0.5,
                    (deco.x1 - deco.x0).max(0.0),
                    deco.thickness,
                ),
                &paint,
            );
        }

        paint.delete();
        font.delete_font();
    }

    fn draw_missing_glyph(&self, run_x: f32, run_y: f32, base: &CkPaint, glyph: &RenderGlyph, font_size: f32) {
        let ph = missing_glyph_placeholder(glyph, font_size);
        if ph.width <= 0.0 || ph.height <= 0.0 {
            return;
        }
        let _ = base;
        // base の色を引き継ぐため作り直す（CkPaint に clone が無い）。TODO(ADR-0148): 色を
        // 引数で持ち回して paint を再利用する。
        let stroke = new_paint(self.ck);
        stroke.set_style(&paint_style_stroke(self.ck));
        stroke.set_stroke_width(ph.stroke_width);
        stroke.set_stroke_cap(&stroke_cap_butt(self.ck));
        stroke.set_stroke_join(&stroke_join_miter(self.ck));
        self.canvas
            .draw_rect(&xywh_rect(self.ck, run_x + ph.x, run_y + ph.y, ph.width, ph.height), &stroke);
        stroke.delete();
    }
}

struct CkPathSink<'a> {
    path: &'a CkPath,
    has_points: bool,
}

impl PathSink for CkPathSink<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.has_points = true;
        self.path.move_to(x, y);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.has_points = true;
        self.path.line_to(x, y);
    }
    fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.has_points = true;
        self.path.quad_to(cx, cy, x, y);
    }
    fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.has_points = true;
        self.path.cubic_to(c1x, c1y, c2x, c2y, x, y);
    }
    fn close(&mut self) {
        self.path.close();
    }
}
