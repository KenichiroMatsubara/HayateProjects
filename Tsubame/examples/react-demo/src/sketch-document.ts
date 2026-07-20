import {
  Paint,
  PaintingStyle,
  Path,
  StrokeCap,
  StrokeJoin,
  type Rgba,
} from '@torimi/tsubame-protocol-generated/recorder';
import type {
  DrawCanvas,
  DrawPainter,
  DrawSize,
} from '@torimi/tsubame-renderer-protocol';

export interface Sample {
  readonly x: number;
  readonly y: number;
}

interface Stroke {
  readonly samples: Sample[];
  readonly color: Rgba;
  readonly width: number;
}

const DEFAULT_INK: Rgba = [0.08, 0.1, 0.14, 1];
const DEFAULT_WIDTH = 5;

export class SketchDocument {
  private readonly committed: Stroke[] = [];
  private inProgress: Stroke | null = null;
  private revision = 0;
  private width = DEFAULT_WIDTH;

  get strokeCount(): number {
    return this.committed.length;
  }

  get isDrawing(): boolean {
    return this.inProgress !== null;
  }

  get strokeWidth(): number {
    return this.width;
  }

  begin(sample: Sample): boolean {
    if (this.inProgress !== null || !validSample(sample)) return false;
    this.inProgress = { samples: [sample], color: DEFAULT_INK, width: this.width };
    this.revision += 1;
    return true;
  }

  append(sample: Sample): boolean {
    const stroke = this.inProgress;
    if (stroke === null || !validSample(sample)) return false;
    const last = stroke.samples.at(-1)!;
    if (Math.hypot(sample.x - last.x, sample.y - last.y) < 0.5) return false;
    stroke.samples.push(sample);
    this.revision += 1;
    return true;
  }

  end(sample: Sample): boolean {
    const stroke = this.inProgress;
    if (stroke === null) return false;
    this.append(sample);
    this.committed.push(stroke);
    this.inProgress = null;
    this.revision += 1;
    return true;
  }

  undo(): boolean {
    if (this.inProgress !== null || this.committed.length === 0) return false;
    this.committed.pop();
    this.revision += 1;
    return true;
  }

  clear(): boolean {
    if (this.inProgress === null && this.committed.length === 0) return false;
    this.committed.length = 0;
    this.inProgress = null;
    this.revision += 1;
    return true;
  }

  setStrokeWidth(width: number): boolean {
    if (!Number.isFinite(width) || width <= 0 || width === this.width) return false;
    this.width = width;
    return true;
  }

  frame(): DrawPainter {
    return new SketchFrame(this, this.revision);
  }

  paint(canvas: DrawCanvas, _size: DrawSize): void {
    for (const stroke of this.committed) paintStroke(canvas, stroke);
    if (this.inProgress !== null) paintStroke(canvas, this.inProgress);
  }
}

class SketchFrame implements DrawPainter {
  constructor(
    private readonly document: SketchDocument,
    private readonly revision: number,
  ) {}

  paint(canvas: DrawCanvas, size: DrawSize): void {
    this.document.paint(canvas, size);
  }

  shouldRepaint(oldPainter: DrawPainter): boolean {
    return !(oldPainter instanceof SketchFrame) || oldPainter.revision !== this.revision;
  }
}

function validSample(sample: Sample): boolean {
  return Number.isFinite(sample.x) && Number.isFinite(sample.y);
}

function paintStroke(canvas: DrawCanvas, stroke: Stroke): void {
  const first = stroke.samples[0];
  if (first === undefined) return;

  const paint = new Paint();
  paint.color = stroke.color;
  paint.style = PaintingStyle.stroke;
  paint.strokeWidth = stroke.width;
  paint.strokeCap = StrokeCap.round;
  paint.strokeJoin = StrokeJoin.round;

  const path = new Path();
  path.moveTo(first.x, first.y);
  if (stroke.samples.length === 1) {
    path.lineTo(first.x + 0.01, first.y + 0.01);
  } else {
    for (let i = 1; i < stroke.samples.length; i += 1) {
      const sample = stroke.samples[i]!;
      path.lineTo(sample.x, sample.y);
    }
  }
  canvas.drawPath(path, paint);
}
