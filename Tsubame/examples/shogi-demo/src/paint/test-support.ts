import type {
  DrawCanvas,
  DrawPaintPacket,
  DrawPaintSource,
  DrawRecordedPath,
  DrawTextStyle,
} from '@torimi/tsubame-renderer-protocol';

/**
 * painter を「観測可能な振る舞い」で検証するための `DrawCanvas` スパイ。
 *
 * painter が**公開契約に対して出した呼び出し列だけ**を記録するので、どちらの
 * レンダラーの内部にも依存しない（recorder の wire バッファでも canvas 2D でもない）。
 * 記録面が wire バッファだと「長さが増えた」程度しか言えず、「駒の漢字が出ている」
 * のような主張が書けない。
 */
export type DrawCall =
  | { op: 'drawPath'; style: number; paint: DrawPaintPacket }
  | {
      op: 'drawText';
      text: string;
      at: readonly [number, number];
      fontSize: number;
      paint: DrawPaintPacket;
    }
  | { op: 'save' }
  | { op: 'restore' }
  | { op: 'translate'; args: readonly [number, number] }
  | { op: 'rotate'; radians: number }
  | { op: 'scale'; args: readonly [number, number] }
  | { op: 'transform'; args: readonly [number, number, number, number, number, number] }
  | { op: 'clipRect'; rect: readonly [number, number, number, number] }
  | { op: 'clipPath' };

/** 座標を変える op（変換）の名前。 */
export const TRANSFORM_OPS: ReadonlySet<string> = new Set([
  'translate',
  'rotate',
  'scale',
  'transform',
]);

export class SpyCanvas implements DrawCanvas {
  readonly calls: DrawCall[] = [];

  /** 記録した `drawText` 呼び出しだけを取り出す。 */
  texts(): Extract<DrawCall, { op: 'drawText' }>[] {
    return this.calls.filter((c) => c.op === 'drawText') as Extract<
      DrawCall,
      { op: 'drawText' }
    >[];
  }

  drawPath(_path: DrawRecordedPath, paint: DrawPaintSource): this {
    this.calls.push({ op: 'drawPath', style: paint.style, paint: paint.toDrawPaint() });
    return this;
  }
  clipPath(): this {
    this.calls.push({ op: 'clipPath' });
    return this;
  }
  drawText(
    text: string,
    x: number,
    y: number,
    paint: DrawPaintSource,
    style?: DrawTextStyle,
  ): this {
    this.calls.push({
      op: 'drawText',
      text,
      at: [x, y],
      fontSize: style?.fontSize ?? 16,
      paint: paint.toDrawPaint(),
    });
    return this;
  }
  save(): this {
    this.calls.push({ op: 'save' });
    return this;
  }
  restore(): this {
    this.calls.push({ op: 'restore' });
    return this;
  }
  translate(dx: number, dy: number): this {
    this.calls.push({ op: 'translate', args: [dx, dy] });
    return this;
  }
  rotate(radians: number): this {
    this.calls.push({ op: 'rotate', radians });
    return this;
  }
  scale(sx: number, sy: number): this {
    this.calls.push({ op: 'scale', args: [sx, sy] });
    return this;
  }
  transform(a: number, b: number, c: number, d: number, e: number, f: number): this {
    this.calls.push({ op: 'transform', args: [a, b, c, d, e, f] });
    return this;
  }
  clipRect(x: number, y: number, width: number, height: number): this {
    this.calls.push({ op: 'clipRect', rect: [x, y, width, height] });
    return this;
  }
}
