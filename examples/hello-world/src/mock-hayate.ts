import {
  OP,
  OP_SLOTS,
  TAG,
  type HayateWasm,
} from '@tsubame/renderer-canvas';

/**
 * デモ用の Hayate WASM スタンドイン。
 *
 * 実 Hayate（Rust → Vello → WebGPU）の代わりに、`apply_mutations` の ops/styles
 * 契約を JS で解釈し 2D Canvas に描画する。これにより本物の WASM が無くても
 * Canvas Renderer 経路をエンドツーエンドで実演でき、Renderer 切替の訴求を体現する。
 *
 * レイアウトは MVP の Flexbox サブセット（column/row・gap・center 系）に絞った
 * 簡易実装であり、Hayate の Taffy ベース実装の完全な再現ではない。
 */

const KIND_NAME = ['view', 'text', 'image', 'button', 'text-input', 'scroll-view'];

interface DecodedStyle {
  width?: number;
  height?: number;
  display?: 'flex' | 'none';
  flexDirection?: 'row' | 'column';
  alignItems?: 'flex-start' | 'flex-end' | 'center' | 'stretch';
  justifyContent?:
    | 'flex-start'
    | 'flex-end'
    | 'center'
    | 'space-between'
    | 'space-around'
    | 'space-evenly';
  gap?: number;
  color?: string;
  backgroundColor?: string;
  borderRadius?: number;
  opacity?: number;
  fontSize?: number;
  fontWeight?: number;
}

interface Node {
  id: number;
  kind: string;
  parent: number | null;
  children: number[];
  style: DecodedStyle;
  text: string;
  rect?: { x: number; y: number; w: number; h: number };
}

const DISPLAY = ['flex', 'none'] as const;
const FLEX_DIRECTION = ['row', 'column'] as const;
const ALIGN_ITEMS = ['flex-start', 'flex-end', 'center', 'stretch'] as const;
const JUSTIFY = [
  'flex-start',
  'flex-end',
  'center',
  'space-between',
  'space-around',
  'space-evenly',
] as const;

const rgba = (r: number, g: number, b: number, a: number): string =>
  `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},${a})`;

export class MockHayate implements HayateWasm {
  private readonly ctx: CanvasRenderingContext2D;
  private readonly nodes = new Map<number, Node>();
  private root: number | null = null;
  private readonly eventQueue: number[] = [];

  constructor(private readonly canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext('2d');
    if (ctx === null) throw new Error('2D context を取得できません');
    this.ctx = ctx;
    canvas.addEventListener('click', this.onClick);
  }

  dispose(): void {
    this.canvas.removeEventListener('click', this.onClick);
  }

  apply_mutations(ops: Float64Array, styles: Float32Array): void {
    let i = 0;
    while (i < ops.length) {
      const op = ops[i]!;
      const slots = OP_SLOTS[op];
      if (slots === undefined) break; // 不明 op は残りを捨てる（ADR-0003）
      switch (op) {
        case OP.CREATE:
          this.nodes.set(ops[i + 1]!, {
            id: ops[i + 1]!,
            kind: KIND_NAME[ops[i + 2]!] ?? 'view',
            parent: null,
            children: [],
            style: {},
            text: '',
          });
          break;
        case OP.SET_ROOT:
          this.root = ops[i + 1]!;
          break;
        case OP.APPEND_CHILD: {
          const parent = this.nodes.get(ops[i + 1]!);
          const child = this.nodes.get(ops[i + 2]!);
          if (parent && child) {
            child.parent = parent.id;
            parent.children.push(child.id);
          }
          break;
        }
        case OP.INSERT_BEFORE: {
          const parent = this.nodes.get(ops[i + 1]!);
          const child = this.nodes.get(ops[i + 2]!);
          if (parent && child) {
            child.parent = parent.id;
            const at = parent.children.indexOf(ops[i + 3]!);
            parent.children.splice(at < 0 ? parent.children.length : at, 0, child.id);
          }
          break;
        }
        case OP.REMOVE: {
          const node = this.nodes.get(ops[i + 1]!);
          if (node && node.parent !== null) {
            const parent = this.nodes.get(node.parent);
            if (parent) {
              const at = parent.children.indexOf(node.id);
              if (at >= 0) parent.children.splice(at, 1);
            }
          }
          break;
        }
        case OP.SET_STYLE:
          this.decodeStyle(ops[i + 1]!, styles, ops[i + 2]!, ops[i + 3]!);
          break;
        default:
          break; // SET_TRANSFORM / SCROLL / FOCUS / BLUR は MVP デモでは未使用
      }
      i += slots;
    }
    this.render();
  }

  element_set_text(id: number, text: string): void {
    const node = this.nodes.get(id);
    if (node) node.text = text;
    this.render();
  }

  poll_events(): Float64Array {
    const out = new Float64Array(this.eventQueue);
    this.eventQueue.length = 0;
    return out;
  }

  private decodeStyle(
    id: number,
    styles: Float32Array,
    offset: number,
    len: number,
  ): void {
    const node = this.nodes.get(id);
    if (!node) return;
    const s = node.style;
    let i = offset;
    const end = offset + len;
    while (i < end) {
      const tag = styles[i++]!;
      const setOp = styles[i++]! === 1;
      const colorTag = tag === TAG.COLOR || tag === TAG.BACKGROUND_COLOR;
      if (!setOp) {
        // reset → 当該プロパティを削除（デフォルトへ）
        delete s[TAG_TO_KEY[tag] as keyof DecodedStyle];
        continue;
      }
      if (colorTag) {
        const value = rgba(styles[i]!, styles[i + 1]!, styles[i + 2]!, styles[i + 3]!);
        i += 4;
        if (tag === TAG.COLOR) s.color = value;
        else s.backgroundColor = value;
        continue;
      }
      const v = styles[i++]!;
      switch (tag) {
        case TAG.WIDTH: s.width = v; break;
        case TAG.HEIGHT: s.height = v; break;
        case TAG.DISPLAY: s.display = DISPLAY[v]; break;
        case TAG.FLEX_DIRECTION: s.flexDirection = FLEX_DIRECTION[v]; break;
        case TAG.ALIGN_ITEMS: s.alignItems = ALIGN_ITEMS[v]; break;
        case TAG.JUSTIFY_CONTENT: s.justifyContent = JUSTIFY[v]; break;
        case TAG.GAP: s.gap = v; break;
        case TAG.BORDER_RADIUS: s.borderRadius = v; break;
        case TAG.OPACITY: s.opacity = v; break;
        case TAG.FONT_SIZE: s.fontSize = v; break;
        case TAG.FONT_WEIGHT: s.fontWeight = v; break;
        default: break;
      }
    }
  }

  // --- レイアウト ---

  private font(s: DecodedStyle): string {
    return `${s.fontWeight ?? 400} ${s.fontSize ?? 16}px system-ui, sans-serif`;
  }

  private isLeafText(node: Node): boolean {
    return (node.kind === 'text' || node.kind === 'button') && node.children.length === 0;
  }

  private padding(node: Node): { x: number; y: number } {
    return node.kind === 'button' ? { x: 16, y: 10 } : { x: 0, y: 0 };
  }

  private measure(node: Node): { w: number; h: number } {
    const s = node.style;
    const pad = this.padding(node);
    let w: number;
    let h: number;
    if (this.isLeafText(node)) {
      this.ctx.font = this.font(s);
      w = this.ctx.measureText(node.text).width + pad.x * 2;
      h = (s.fontSize ?? 16) * 1.4 + pad.y * 2;
    } else {
      const kids = node.children
        .map((id) => this.nodes.get(id))
        .filter((n): n is Node => n !== undefined && n.style.display !== 'none')
        .map((n) => ({ n, size: this.measure(n) }));
      const gap = s.gap ?? 0;
      const row = (s.flexDirection ?? 'row') === 'row';
      const main = kids.reduce((sum, k) => sum + (row ? k.size.w : k.size.h), 0) +
        gap * Math.max(0, kids.length - 1);
      const cross = kids.reduce((m, k) => Math.max(m, row ? k.size.h : k.size.w), 0);
      w = (row ? main : cross) + pad.x * 2;
      h = (row ? cross : main) + pad.y * 2;
    }
    if (s.width !== undefined) w = s.width;
    if (s.height !== undefined) h = s.height;
    return { w, h };
  }

  private place(node: Node, x: number, y: number, w: number, h: number): void {
    node.rect = { x, y, w, h };
    if (this.isLeafText(node)) return;
    const s = node.style;
    const pad = this.padding(node);
    const innerX = x + pad.x;
    const innerY = y + pad.y;
    const innerW = w - pad.x * 2;
    const innerH = h - pad.y * 2;
    const row = (s.flexDirection ?? 'row') === 'row';
    const gap = s.gap ?? 0;

    const kids = node.children
      .map((id) => this.nodes.get(id))
      .filter((n): n is Node => n !== undefined && n.style.display !== 'none')
      .map((n) => ({ n, size: this.measure(n) }));

    const mainTotal = kids.reduce((sum, k) => sum + (row ? k.size.w : k.size.h), 0) +
      gap * Math.max(0, kids.length - 1);
    const mainAvail = row ? innerW : innerH;
    const justify = s.justifyContent ?? 'flex-start';
    let cursor = row ? innerX : innerY;
    if (justify === 'center') cursor += (mainAvail - mainTotal) / 2;
    else if (justify === 'flex-end') cursor += mainAvail - mainTotal;

    const align = s.alignItems ?? 'stretch';
    for (const { n, size } of kids) {
      const crossAvail = row ? innerH : innerW;
      const crossSize = row ? size.h : size.w;
      let crossPos = row ? innerY : innerX;
      if (align === 'center') crossPos += (crossAvail - crossSize) / 2;
      else if (align === 'flex-end') crossPos += crossAvail - crossSize;

      if (row) {
        this.place(n, cursor, crossPos, size.w, align === 'stretch' ? innerH : size.h);
        cursor += size.w + gap;
      } else {
        this.place(n, crossPos, cursor, align === 'stretch' ? innerW : size.w, size.h);
        cursor += size.h + gap;
      }
    }
  }

  // --- 描画 ---

  private render(): void {
    const { ctx, canvas } = this;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (this.root === null) return;
    const rootNode = this.nodes.get(this.root);
    if (!rootNode) return;
    const size = this.measure(rootNode);
    // ルートはキャンバス中央に配置。
    const x = (canvas.width - size.w) / 2;
    const y = (canvas.height - size.h) / 2;
    this.place(rootNode, x, y, size.w, size.h);
    this.draw(rootNode);
  }

  private draw(node: Node): void {
    const r = node.rect;
    if (!r || node.style.display === 'none') return;
    const { ctx } = this;
    const s = node.style;
    ctx.save();
    ctx.globalAlpha = s.opacity ?? 1;

    if (s.backgroundColor) {
      this.roundRect(r.x, r.y, r.w, r.h, s.borderRadius ?? 0);
      ctx.fillStyle = s.backgroundColor;
      ctx.fill();
    }

    if (this.isLeafText(node)) {
      ctx.font = this.font(s);
      ctx.fillStyle = s.color ?? '#000';
      ctx.textBaseline = 'middle';
      ctx.textAlign = 'center';
      ctx.fillText(node.text, r.x + r.w / 2, r.y + r.h / 2);
    }
    ctx.restore();

    for (const id of node.children) {
      const child = this.nodes.get(id);
      if (child) this.draw(child);
    }
  }

  private roundRect(x: number, y: number, w: number, h: number, radius: number): void {
    const r = Math.min(radius, w / 2, h / 2);
    const { ctx } = this;
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.arcTo(x + w, y, x + w, y + h, r);
    ctx.arcTo(x + w, y + h, x, y + h, r);
    ctx.arcTo(x, y + h, x, y, r);
    ctx.arcTo(x, y, x + w, y, r);
    ctx.closePath();
  }

  // --- ヒットテスト（click） ---

  private readonly onClick = (e: MouseEvent): void => {
    const rect = this.canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    const hit = this.hitTest(px, py);
    if (hit !== null) {
      this.eventQueue.push(0 /* click */, hit);
    }
  };

  /** 点を含む最も深い（手前の）ノード id を返す。 */
  private hitTest(px: number, py: number): number | null {
    let found: number | null = null;
    const visit = (node: Node): void => {
      const r = node.rect;
      if (!r || node.style.display === 'none') return;
      if (px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h) {
        found = node.id; // 後勝ち＝より深いノードで上書き
      }
      for (const id of node.children) {
        const child = this.nodes.get(id);
        if (child) visit(child);
      }
    };
    if (this.root !== null) {
      const rootNode = this.nodes.get(this.root);
      if (rootNode) visit(rootNode);
    }
    return found;
  }
}

const TAG_TO_KEY: Record<number, string> = {
  [TAG.WIDTH]: 'width',
  [TAG.HEIGHT]: 'height',
  [TAG.DISPLAY]: 'display',
  [TAG.FLEX_DIRECTION]: 'flexDirection',
  [TAG.ALIGN_ITEMS]: 'alignItems',
  [TAG.JUSTIFY_CONTENT]: 'justifyContent',
  [TAG.GAP]: 'gap',
  [TAG.COLOR]: 'color',
  [TAG.BACKGROUND_COLOR]: 'backgroundColor',
  [TAG.BORDER_RADIUS]: 'borderRadius',
  [TAG.OPACITY]: 'opacity',
  [TAG.FONT_SIZE]: 'fontSize',
  [TAG.FONT_WEIGHT]: 'fontWeight',
};
