import {
  Paint,
  Path,
  PaintingStyle,
  StrokeCap,
  StrokeJoin,
} from '@torimi/tsubame-protocol-generated/recorder';
import type { DrawCanvas, DrawPaintFunction, DrawSize } from '@torimi/tsubame-renderer-protocol';
import type { Palette } from '../theme';
import type { Priority } from '../todo-model';

type Rgba = readonly [number, number, number, number];

function rgba(hex: string, alpha = 1): Rgba {
  const raw = hex.replace('#', '').slice(0, 6);
  return [
    Number.parseInt(raw.slice(0, 2), 16) / 255,
    Number.parseInt(raw.slice(2, 4), 16) / 255,
    Number.parseInt(raw.slice(4, 6), 16) / 255,
    alpha,
  ];
}

function fill(color: string, alpha = 1): Paint {
  const paint = new Paint();
  paint.color = rgba(color, alpha);
  return paint;
}

function stroke(color: string, width: number, alpha = 1): Paint {
  const paint = fill(color, alpha);
  paint.style = PaintingStyle.stroke;
  paint.strokeWidth = width;
  paint.strokeCap = StrokeCap.round;
  paint.strokeJoin = StrokeJoin.round;
  return paint;
}

function circle(canvas: DrawCanvas, x: number, y: number, radius: number, paint: Paint): void {
  const path = new Path();
  path.addCircle(x, y, radius);
  canvas.drawPath(path, paint);
}

/** A — 時間軸ではなく、流れそのものをUIにするための軌道面。 */
export function orbitPainter(colors: Palette, completed: number, total: number): DrawPaintFunction {
  return (canvas: DrawCanvas, size: DrawSize) => {
    const w = size.width;
    const h = size.height;
    const wash = new Path();
    wash.addRRect(0, 0, w, h, 28, 28);
    canvas.drawPath(wash, fill(colors.panel));

    for (let i = 0; i < 5; i++) {
      const ring = new Path();
      ring.addOval(w * 0.52 - 150 - i * 26, h * 0.43 - 70 - i * 15, 300 + i * 52, 140 + i * 30);
      canvas.drawPath(ring, stroke(colors.accent, i === 0 ? 2.5 : 1, 0.11 + i * 0.025));
    }

    const orbit = new Path();
    orbit.moveTo(-20, h * 0.76);
    orbit.cubicTo(w * 0.17, h * 0.72, w * 0.12, h * 0.23, w * 0.39, h * 0.34);
    orbit.cubicTo(w * 0.67, h * 0.45, w * 0.67, h * 0.08, w + 30, h * 0.22);
    canvas.drawPath(orbit, stroke(colors.accent, 5, 0.95));
    canvas.drawPath(orbit, stroke(colors.ink, 13, 0.055));

    const count = Math.max(total, 4);
    for (let i = 0; i < count; i++) {
      const t = i / Math.max(1, count - 1);
      const x = 44 + t * (w - 88);
      const y = h * (0.65 - 0.34 * Math.sin(t * Math.PI));
      circle(canvas, x, y, i < completed ? 10 : 7, fill(i < completed ? colors.success : colors.panel3));
      circle(canvas, x, y, i < completed ? 16 : 12, stroke(i < completed ? colors.success : colors.accent, 2, 0.42));
    }

    const comet = new Path();
    comet.moveTo(w * 0.78, h * 0.13);
    comet.cubicTo(w * 0.88, h * 0.16, w * 0.91, h * 0.28, w * 1.04, h * 0.29);
    canvas.drawPath(comet, stroke(colors.accent2, 3, 0.75));
    circle(canvas, w * 0.78, h * 0.13, 6, fill(colors.accent2));
  };
}

/** B — 完了率を数値でなく、呼吸するようなセグメントの密度で見せる。 */
export function focusOrbPainter(colors: Palette, percent: number): DrawPaintFunction {
  return (canvas: DrawCanvas, size: DrawSize) => {
    const cx = size.width / 2;
    const cy = size.height / 2;
    const radius = Math.min(size.width, size.height) * 0.34;
    circle(canvas, cx, cy, radius * 1.34, fill(colors.accent, 0.045));
    circle(canvas, cx, cy, radius * 1.12, stroke(colors.accent, 1, 0.2));
    circle(canvas, cx, cy, radius * 0.82, fill(colors.panel2, 0.72));

    const segments = 40;
    const active = Math.round((segments * percent) / 100);
    for (let i = 0; i < segments; i++) {
      const angle = -Math.PI / 2 + (i / segments) * Math.PI * 2;
      const x = cx + Math.cos(angle) * radius;
      const y = cy + Math.sin(angle) * radius;
      const on = i < active;
      circle(canvas, x, y, on ? 4.8 : 2.6, fill(on ? colors.accent : colors.line, on ? 1 : 0.65));
    }

    for (let i = 0; i < 12; i++) {
      const angle = i * 2.399;
      const r = radius * (1.25 + (i % 3) * 0.12);
      circle(canvas, cx + Math.cos(angle) * r, cy + Math.sin(angle) * r, i % 4 === 0 ? 3 : 1.5, fill(colors.accent2, 0.45));
    }
  };
}

export const CONSTELLATION_POINTS = [
  { x: 15, y: 26 },
  { x: 40, y: 16 },
  { x: 69, y: 27 },
  { x: 82, y: 57 },
  { x: 57, y: 72 },
  { x: 28, y: 67 },
] as const;

function priorityColor(colors: Palette, priority: Priority): string {
  if (priority === 1) return colors.danger;
  if (priority === 2) return colors.accent2;
  return colors.blue;
}

/** C — タスク同士を孤立した行ではなく、関連する星群として扱う背景。 */
export function constellationPainter(
  colors: Palette,
  priorities: readonly Priority[],
  done: readonly boolean[],
): DrawPaintFunction {
  return (canvas: DrawCanvas, size: DrawSize) => {
    const bg = new Path();
    bg.addRRect(0, 0, size.width, size.height, 28, 28);
    canvas.drawPath(bg, fill(colors.black));

    for (let i = 0; i < 34; i++) {
      const x = ((i * 73) % 101) / 100 * size.width;
      const y = ((i * 47 + 13) % 97) / 96 * size.height;
      circle(canvas, x, y, i % 7 === 0 ? 1.8 : 0.8, fill(colors.ink, i % 7 === 0 ? 0.48 : 0.2));
    }

    const count = Math.min(priorities.length, CONSTELLATION_POINTS.length);
    if (count > 1) {
      const path = new Path();
      for (let i = 0; i < count; i++) {
        const point = CONSTELLATION_POINTS[i]!;
        const x = point.x / 100 * size.width;
        const y = point.y / 100 * size.height;
        if (i === 0) path.moveTo(x, y);
        else path.lineTo(x, y);
      }
      canvas.drawPath(path, stroke(colors.violet, 2, 0.42));
    }

    for (let i = 0; i < count; i++) {
      const point = CONSTELLATION_POINTS[i]!;
      const x = point.x / 100 * size.width;
      const y = point.y / 100 * size.height;
      const tone = done[i] ? colors.success : priorityColor(colors, priorities[i]!);
      circle(canvas, x, y, done[i] ? 8 : 11, fill(tone, 0.98));
      circle(canvas, x, y, done[i] ? 18 : 25, fill(tone, 0.08));
      circle(canvas, x, y, done[i] ? 13 : 18, stroke(tone, 1.5, 0.5));
    }
  };
}
