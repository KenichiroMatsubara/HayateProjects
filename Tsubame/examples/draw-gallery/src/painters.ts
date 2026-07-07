import {
  Path,
  Paint,
  PaintingStyle,
  PathFillType,
  StrokeCap,
  StrokeJoin,
} from '@tsubame/protocol-generated/recorder';
import type { DrawCanvas, DrawSize } from '@tsubame/renderer-protocol';

/**
 * draw ギャラリーのサンプル painter 群（issue #732）。draw v1 語彙を横断する
 * 小さな painter を、フレームワーク非依存・レンダラー非依存の純関数として置く。
 * 各 painter は `(canvas, size)` を受け取り DrawCanvas 契約だけに触れるので、
 * 同一関数が Hayate Renderer（wire 記録）と DOM Renderer（canvas 2D replay）の
 * 両経路で同じ絵を出す。
 */

/** 折れ線チャートの正規化サンプル（0..1）。box に合わせてスケールする。 */
const CHART_SERIES: readonly number[] = [0.15, 0.55, 0.3, 0.8, 0.45, 0.95, 0.6];

/**
 * cubic bezier の曲線チャート。系列の各サンプルを box サイズへスケールし、
 * 隣接点を滑らかな 3 次ベジェで繋ぐ（Catmull-Rom 風の制御点）。resize で
 * 制御点が引き伸ばされる典型を示す。
 */
export function curveChart(canvas: DrawCanvas, size: DrawSize): void {
  const pad = Math.min(size.width, size.height) * 0.08;
  const innerW = Math.max(0, size.width - pad * 2);
  const innerH = Math.max(0, size.height - pad * 2);
  const n = CHART_SERIES.length;
  const xAt = (i: number): number => pad + (innerW * i) / (n - 1);
  const yAt = (i: number): number => pad + innerH * (1 - CHART_SERIES[i]!);

  const path = new Path();
  path.moveTo(xAt(0), yAt(0));
  for (let i = 0; i < n - 1; i++) {
    // 制御点は水平方向に区間幅の 1/3 ずらす（滑らかな S 字）。
    const dx = (xAt(i + 1) - xAt(i)) / 3;
    path.cubicTo(xAt(i) + dx, yAt(i), xAt(i + 1) - dx, yAt(i + 1), xAt(i + 1), yAt(i + 1));
  }

  const paint = new Paint();
  paint.style = PaintingStyle.stroke;
  paint.color = [0.16, 0.72, 0.65, 1];
  paint.strokeWidth = Math.max(2, Math.min(size.width, size.height) * 0.03);
  canvas.drawPath(path, paint);
}

/**
 * evenOdd 塗り規則でドーナツ（中抜き円）を描く。外周円と内周円を 1 パスに入れ、
 * fillType=evenOdd で内側を打ち抜く。nonZero では両円が同巻きで潰れて穴が
 * 出ないため、evenOdd 規則そのものの実証になる。
 */
export function evenOddDonut(canvas: DrawCanvas, size: DrawSize): void {
  const cx = size.width / 2;
  const cy = size.height / 2;
  const outer = Math.min(size.width, size.height) * 0.42;
  const inner = outer * 0.55;

  const path = new Path();
  path.addCircle(cx, cy, outer);
  path.addCircle(cx, cy, inner);

  const paint = new Paint();
  paint.style = PaintingStyle.fill;
  paint.fillType = PathFillType.evenOdd;
  paint.color = [0.9, 0.4, 0.35, 1];
  canvas.drawPath(path, paint);
}

/** dash + cap/join 見本の 1 行の仕様。 */
interface StrokeSample {
  readonly cap: StrokeCap;
  readonly join: StrokeJoin;
  readonly dash: readonly number[];
}

const STROKE_SAMPLES: readonly StrokeSample[] = [
  { cap: StrokeCap.butt, join: StrokeJoin.miter, dash: [] },
  { cap: StrokeCap.round, join: StrokeJoin.round, dash: [12, 8] },
  { cap: StrokeCap.square, join: StrokeJoin.bevel, dash: [4, 6] },
];

/**
 * 破線・cap・join の見本。各行に「く」の字の折れ線を引き、行ごとに cap / join /
 * dash を変えて太いストロークの端点・角の見た目の違いを並べる。太い width で
 * cap/join の差が目視できるようにする。
 */
export function dashStrokeSampler(canvas: DrawCanvas, size: DrawSize): void {
  const rows = STROKE_SAMPLES.length;
  const marginX = size.width * 0.12;
  const rowH = size.height / (rows + 1);
  const width = Math.max(6, size.height * 0.06);

  STROKE_SAMPLES.forEach((sample, i) => {
    const y = rowH * (i + 1);
    const x0 = marginX;
    const x1 = size.width - marginX;
    const mid = (x0 + x1) / 2;
    // 「く」の字: 角を作って join を見せる。
    const path = new Path();
    path.moveTo(x0, y + rowH * 0.28);
    path.lineTo(mid, y - rowH * 0.28);
    path.lineTo(x1, y + rowH * 0.28);

    const paint = new Paint();
    paint.style = PaintingStyle.stroke;
    paint.color = [0.4, 0.55, 0.95, 1];
    paint.strokeWidth = width;
    paint.strokeCap = sample.cap;
    paint.strokeJoin = sample.join;
    paint.dash = sample.dash;
    canvas.drawPath(path, paint);
  });
}

/**
 * 回転 + クリップの組み合わせ。box 中心で座標系を回し、その回転済み座標で
 * 矩形クリップを張ってから斜めストライプを敷き詰める。ストライプはクリップ窓の
 * 外へ伸びるが、回転した矩形の内側だけが見える（save/restore でクリップと変換を
 * 局所化する）。
 */
export function rotatedClip(canvas: DrawCanvas, size: DrawSize): void {
  const cx = size.width / 2;
  const cy = size.height / 2;
  const half = Math.min(size.width, size.height) * 0.36;

  canvas.save();
  canvas.translate(cx, cy);
  canvas.rotate(Math.PI / 6); // 30°
  canvas.clipRect(-half, -half, half * 2, half * 2);

  // クリップ窓を斜めに横切るストライプ。窓より広い範囲に引いてはみ出しを作る。
  const span = half * 3;
  const step = Math.max(8, half * 0.22);
  const paint = new Paint();
  paint.style = PaintingStyle.fill;
  paint.color = [0.55, 0.42, 0.95, 1];
  for (let x = -span; x < span; x += step * 2) {
    const stripe = new Path();
    stripe.addRect(x, -span, step, span * 2);
    canvas.drawPath(stripe, paint);
  }

  canvas.restore();
}

/** グリッドの基準セル寸法（論理 px）。box が広がるとセル数が増える。 */
const GRID_CELL_PX = 44;

/**
 * サイズ追従 painter。固定セル寸法（{@link GRID_CELL_PX}）の市松模様を敷くので、
 * box が広がるほどセル数が増える — 単なる拡大縮小ではなく「絵そのものが
 * サイズで変わる」。resize→paint ループの実地デモ。0 サイズ（初回レイアウト前）
 * では何も描かない。
 */
export function responsiveGrid(canvas: DrawCanvas, size: DrawSize): void {
  const cols = Math.floor(size.width / GRID_CELL_PX);
  const rows = Math.floor(size.height / GRID_CELL_PX);
  if (cols <= 0 || rows <= 0) return;

  const cellW = size.width / cols;
  const cellH = size.height / rows;
  const paint = new Paint();
  paint.style = PaintingStyle.fill;

  for (let r = 0; r < rows; r++) {
    for (let c = 0; c < cols; c++) {
      if ((r + c) % 2 === 0) continue; // 市松: 片側だけ塗る。
      const cell = new Path();
      cell.addRect(c * cellW, r * cellH, cellW, cellH);
      // 位置に応じて色相をずらし、グリッドの広がりが目に見えるようにする。
      const t = (r + c) / (rows + cols);
      paint.color = [0.2 + 0.6 * t, 0.75 - 0.4 * t, 0.85, 1];
      canvas.drawPath(cell, paint);
    }
  }
}

/** ギャラリーの 1 枚のカード記述子。App / e2e はこの id で painter を列挙する。 */
export interface GalleryPainter {
  /** 安定した slug（data-testid / e2e locator の正本）。 */
  readonly id: string;
  readonly title: string;
  readonly blurb: string;
  readonly paint: (canvas: DrawCanvas, size: DrawSize) => void;
}

/**
 * ギャラリーが横断展示する painter 群（受け入れ基準の 5 種）。App はこの配列を
 * map してカードを敷き、両レンダラーとも同じ順序・同じ id で描く。
 */
export const GALLERY_PAINTERS: readonly GalleryPainter[] = [
  {
    id: 'curve-chart',
    title: 'Cubic Bézier chart',
    blurb: '3 次ベジェで結んだ折れ線チャート',
    paint: curveChart,
  },
  {
    id: 'even-odd-donut',
    title: 'evenOdd donut',
    blurb: 'evenOdd 塗り規則で中抜きした円',
    paint: evenOddDonut,
  },
  {
    id: 'dash-sampler',
    title: 'Dash + cap/join',
    blurb: '破線・線端・角の見本',
    paint: dashStrokeSampler,
  },
  {
    id: 'rotated-clip',
    title: 'Rotate + clip',
    blurb: '回転した矩形でクリップした斜めストライプ',
    paint: rotatedClip,
  },
  {
    id: 'responsive-grid',
    title: 'Size-following grid',
    blurb: 'box が広がるとセル数が増える市松模様',
    paint: responsiveGrid,
  },
];
