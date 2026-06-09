/** Minimal apply_mutations batch helper for web-demo (ADR-0052 / ADR-0072). */

export const TAG = {
  BACKGROUND_COLOR: 0,
  OPACITY: 1,
  BORDER_RADIUS: 2,
  WIDTH: 5,
  HEIGHT: 6,
  DISPLAY: 11,
  FLEX_DIRECTION: 12,
  ALIGN_ITEMS: 13,
  JUSTIFY_CONTENT: 14,
  GAP: 15,
  PADDING: 16,
  FONT_SIZE: 26,
  COLOR: 27,
};

export const UNIT = { PX: 0, PERCENT: 1, AUTO: 2, FR: 3 };
export const DISPLAY = { FLEX: 0, GRID: 1, BLOCK: 2, NONE: 3 };

export const OP = {
  APPEND_CHILD: 0,
  REMOVE: 2,
  SET_ROOT: 3,
  SET_STYLE: 4,
};

export class HayateBatch {
  constructor() {
    this.ops = [];
    this.styles = [];
  }

  setRoot(id) {
    this.ops.push(OP.SET_ROOT, id);
  }

  appendChild(parent, child) {
    this.ops.push(OP.APPEND_CHILD, parent, child);
  }

  remove(id) {
    this.ops.push(OP.REMOVE, id);
  }

  setStyle(id, ...slots) {
    const offset = this.styles.length;
    this.styles.push(...slots);
    this.ops.push(OP.SET_STYLE, id, offset, slots.length);
  }

  flush(renderer) {
    if (this.ops.length === 0) return;
    renderer.apply_mutations(
      new Float64Array(this.ops),
      new Float32Array(this.styles),
    );
    this.ops = [];
    this.styles = [];
  }
}

export function hexToRgb01(hex) {
  return [
    parseInt(hex.slice(1, 3), 16) / 255,
    parseInt(hex.slice(3, 5), 16) / 255,
    parseInt(hex.slice(5, 7), 16) / 255,
  ];
}
