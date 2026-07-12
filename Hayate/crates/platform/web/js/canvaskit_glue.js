// ADR-0148 — Skia CanvasKit backend の JS グルー（DRAFT / Phase 2、未検証）
//
// CanvasKit（Google Skia の公式 wasm ビルド、npm `canvaskit-wasm`）は自前の非同期
// ローダ（`CanvasKitInit`）で ~7MB の wasm を読み込む。wasm-bindgen backend
// （`backend/canvaskit_backend.rs`）はこのモジュールの関数を extern 経由で呼ぶ。
//
// ここが CanvasKit との唯一の JS 境界。Rust 側は new 演算子/プロパティアクセスを
// 直接持てないため、CanvasKit オブジェクトの生成（Paint/Path/Font 等）と enum 値の
// 取得だけをここで薄くラップし、描画メソッドは Rust から CanvasKit オブジェクトへ
// 直接呼ぶ（extern method binding、painter は Rust に留める）。
//
// 前提（Phase 2 の結線対象・未実装）:
//  - アプリの bundler（Vite）が `canvaskit-wasm` を解決し、`canvaskit.wasm` を配信する
//    （`locateFile` がその URL を返す）。deploy-pages に asset 配信を足す（ADR-0148 §Consequences）。
//  - この .js は wasm-bindgen が出力へコピーする（`#[wasm_bindgen(module = "/js/canvaskit_glue.js")]`）。

// eslint-disable-next-line import/no-unresolved -- Phase 2 で app 依存として解決される
import CanvasKitInit from 'canvaskit-wasm';

let cachedCanvasKit = null;

/**
 * CanvasKit を一度だけ初期化して返す。`locateBase` は `canvaskit.wasm` を配信する
 * ベース URL（末尾スラッシュ込み）。
 */
export async function loadCanvasKit(locateBase) {
  if (cachedCanvasKit) return cachedCanvasKit;
  cachedCanvasKit = await CanvasKitInit({
    locateFile: (file) => `${locateBase ?? ''}${file}`,
  });
  return cachedCanvasKit;
}

/**
 * 対象 canvas 上に Skia surface を作る。GPU（WebGL）を優先し、取れなければ CPU（SW）へ
 * フォールバックする（CanvasKit 自体の GL/SW 二段構え。vello の wgpu とは別経路）。
 */
export function makeSurface(CanvasKit, canvas) {
  const gpu = CanvasKit.MakeWebGLCanvasSurface(canvas);
  if (gpu) return gpu;
  // TODO(ADR-0148 Phase 2): SW surface は width/height 明示が要る版がある。要実機確認。
  return CanvasKit.MakeSWCanvasSurface(canvas);
}

// ── 生成ヘルパ（Rust から `new CanvasKit.*()` を呼べないための薄いラッパ） ──────────

export function newPaint(CanvasKit) {
  const paint = new CanvasKit.Paint();
  paint.setAntiAlias(true);
  return paint;
}

export function newPath(CanvasKit) {
  return new CanvasKit.Path();
}

export function newFont(CanvasKit, typeface, size) {
  return new CanvasKit.Font(typeface, size);
}

export function color4f(CanvasKit, r, g, b, a) {
  return CanvasKit.Color4f(r, g, b, a);
}

export function xywhRect(CanvasKit, x, y, w, h) {
  return CanvasKit.XYWHRect(x, y, w, h);
}

/** 一様角丸の RRect（LTRB rect + rx/ry）。 */
export function rrectXY(CanvasKit, x, y, w, h, radius) {
  return CanvasKit.RRectXY(CanvasKit.XYWHRect(x, y, w, h), radius, radius);
}

export function makeDashPathEffect(CanvasKit, on, off, phase) {
  return CanvasKit.PathEffect.MakeDash([on, off], phase);
}

export function makeTypefaceFromData(CanvasKit, bytes) {
  // TODO(ADR-0148 Phase 4): variable font の variation 座標焼き込みは CanvasKit の
  // Font.setVariations / Typeface variation API 経由。ネイティブ skia の avar 逆写像
  // （painter.rs design_coords_from_normalized）に相当する処理をここか Rust 側で行う。
  return CanvasKit.Typeface.MakeFreeTypeFaceFromData(bytes.buffer ?? bytes);
}

export function makeImage(CanvasKit, bytes, width, height, alphaType) {
  const info = {
    width,
    height,
    colorType: CanvasKit.ColorType.RGBA_8888,
    alphaType,
    colorSpace: CanvasKit.ColorSpace.SRGB,
  };
  return CanvasKit.MakeImage(info, bytes, width * 4);
}

// ── enum 値（Rust から property を読めないためゲッタで露出） ────────────────────────

export const paintStyleFill = (CanvasKit) => CanvasKit.PaintStyle.Fill;
export const paintStyleStroke = (CanvasKit) => CanvasKit.PaintStyle.Stroke;
export const strokeCapButt = (CanvasKit) => CanvasKit.StrokeCap.Butt;
export const strokeCapRound = (CanvasKit) => CanvasKit.StrokeCap.Round;
export const strokeCapSquare = (CanvasKit) => CanvasKit.StrokeCap.Square;
export const strokeJoinMiter = (CanvasKit) => CanvasKit.StrokeJoin.Miter;
export const strokeJoinRound = (CanvasKit) => CanvasKit.StrokeJoin.Round;
export const strokeJoinBevel = (CanvasKit) => CanvasKit.StrokeJoin.Bevel;
export const fillTypeWinding = (CanvasKit) => CanvasKit.FillType.Winding;
export const fillTypeEvenOdd = (CanvasKit) => CanvasKit.FillType.EvenOdd;
export const alphaTypeOpaque = (CanvasKit) => CanvasKit.AlphaType.Opaque;
export const alphaTypePremul = (CanvasKit) => CanvasKit.AlphaType.Premul;
export const alphaTypeUnpremul = (CanvasKit) => CanvasKit.AlphaType.Unpremul;
export const clipOpIntersect = (CanvasKit) => CanvasKit.ClipOp.Intersect;
