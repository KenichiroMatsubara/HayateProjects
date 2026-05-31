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
 * レイアウトは MVP の Flexbox サブセット（column/row・gap・alignItems・flexGrow）に絞った
 * 簡易実装であり、Hayate の Taffy ベース実装の完全な再現ではない。
 */

const KIND_NAME = ['view', 'text', 'image', 'button', 'text-input', 'scroll-view'];

interface DecodedStyle {
  width?: number | string;   // number=px, string='100%' など
  height?: number | string;
  flexGrow?: number;
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

  /** Canvas サーフェスのサイズを更新して再描画する（IRenderer.resize 経由で呼ばれる）。 */
  resize(width: number, height: number): void {
    this.canvas.width = width;
    this.canvas.height = height;
    this.canvas.style.width = `${width}px`;
    this.canvas.style.height = `${height}px`;
    this.render();
  }

  apply_mutations(ops: Float64Array, styles: Float32Array): void {
    let i = 0;
    while (i < ops.length) {
      const op = ops[i]!;
      const slots = OP_SLOTS[op];
      if (slots === undefined) break;
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
          break;
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

  poll_events(): Array<Array<number | string>> {
    const result: Array<[number, number]> = [];
    for (let i = 0; i + 1 < this.eventQueue.length; i += 2) {
      result.push([this.eventQueue[i]!, this.eventQueue[i + 1]!]);
    }
    this.eventQueue.length = 0;
    return result;
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
      if (tag === TAG.COLOR || tag === TAG.BACKGROUND_COLOR) {
        const value = rgba(styles[i]!, styles[i + 1]!, styles[i + 2]!, styles[i + 3]!);
        i += 4;
        if (tag === TAG.COLOR) s.color = value;
        else s.backgroundColor = value;
      } else if (tag === TAG.WIDTH || tag === TAG.HEIGHT || tag === TAG.GAP) {
        const v = styles[i++]!;
        const unit = styles[i++]!; // 0=px, 1=percent
        if (tag === TAG.WIDTH) s.width = unit === 1 ? `${v}%` : v;
        else if (tag === TAG.HEIGHT) s.height = unit === 1 ? `${v}%` : v;
        else s.gap = v; // gap は常に px
      } else {
        const v = styles[i++]!;
        switch (tag) {
          case TAG.DISPLAY: s.display = DISPLAY[v]; break;
          case TAG.FLEX_DIRECTION: s.flexDirection = FLEX_DIRECTION[v]; break;
          case TAG.ALIGN_ITEMS: s.alignItems = ALIGN_ITEMS[v]; break;
          case TAG.JUSTIFY_CONTENT: s.justifyContent = JUSTIFY[v]; break;
          case TAG.BORDER_RADIUS: s.borderRadius = v; break;
          case TAG.OPACITY: s.opacity = v; break;
          case TAG.FONT_SIZE: s.fontSize = v; break;
          case TAG.FLEX_GROW: s.flexGrow = v; break;
          default: break;
        }
      }
    }
  }

  // --- レイアウト ---

  private font(s: DecodedStyle): string {
    return `${s.fontSize ?? 16}px system-ui, sans-serif`;
  }

  private isLeafText(node: Node): boolean {
    return (node.kind === 'text' || node.kind === 'button') && node.children.length === 0;
  }

  private padding(node: Node): { x: number; y: number } {
    return node.kind === 'button' ? { x: 16, y: 10 } : { x: 0, y: 0 };
  }

  /**
   * ノードの内在サイズを計算する（ボトムアップ）。
   * - `%` 指定・`flexGrow` は考慮しない（親サイズが未確定のため）
   * - 明示 px 寸法がある場合はそれを使用
   */
  private measure(node: Node): { w: number; h: number } {
    const s = node.style;
    const pad = this.padding(node);
    let w = 0;
    let h = 0;

    if (this.isLeafText(node)) {
      this.ctx.font = this.font(s);
      w = this.ctx.measureText(node.text).width + pad.x * 2;
      h = (s.fontSize ?? 16) * 1.4 + pad.y * 2;
    } else {
      const kids = node.children
        .map((id) => this.nodes.get(id))
        .filter((n): n is Node => n !== undefined && n.style.display !== 'none')
        // flexGrow 持ちの子は内在サイズ計算から除外（親が割り当てる）
        .filter((n) => (n.style.flexGrow ?? 0) === 0);

      const row = (s.flexDirection ?? 'row') === 'row';
      const gap = s.gap ?? 0;
      const sizes = kids.map((k) => this.measure(k));
      const mainTotal =
        sizes.reduce((sum, sz) => sum + (row ? sz.w : sz.h), 0) +
        gap * Math.max(0, kids.length - 1);
      const crossMax = sizes.reduce((m, sz) => Math.max(m, row ? sz.h : sz.w), 0);
      w = (row ? mainTotal : crossMax) + pad.x * 2;
      h = (row ? crossMax : mainTotal) + pad.y * 2;
    }

    // 明示 px 寸法で上書き（'%' は無視して親任せ）
    if (typeof s.width === 'number') w = s.width;
    if (typeof s.height === 'number') h = s.height;
    return { w, h };
  }

  /**
   * ノードを指定矩形に配置する（トップダウン）。
   * - 渡された (w, h) がこのノードの確定サイズ
   * - 子の `flexGrow` 分の空きを計算して分配する
   * - `%` 指定は親サイズで解決する
   */
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
    const align = s.alignItems ?? 'stretch';
    const justify = s.justifyContent ?? 'flex-start';

    const kids = node.children
      .map((id) => this.nodes.get(id))
      .filter((n): n is Node => n !== undefined && n.style.display !== 'none');

    if (kids.length === 0) return;

    const mainAvail = row ? innerW : innerH;

    // 各子の内在サイズと flexGrow を収集
    const kidData = kids.map((k) => ({
      node: k,
      size: this.measure(k),
      grow: k.style.flexGrow ?? 0,
    }));

    const totalGrow = kidData.reduce((sum, d) => sum + d.grow, 0);
    const gapsTotal = gap * Math.max(0, kids.length - 1);

    // flexGrow を持たない子の主軸合計
    const fixedMain = kidData
      .filter((d) => d.grow === 0)
      .reduce((sum, d) => sum + (row ? d.size.w : d.size.h), 0);

    const remaining = Math.max(0, mainAvail - fixedMain - gapsTotal);

    // justifyContent オフセット（flexGrow がある場合は無効）
    let cursor = row ? innerX : innerY;
    let extraGap = gap;
    if (totalGrow === 0) {
      const mainTotal = fixedMain + gapsTotal;
      const free = mainAvail - mainTotal;
      if (justify === 'center') {
        cursor += free / 2;
      } else if (justify === 'flex-end') {
        cursor += free;
      } else if (justify === 'space-between' && kids.length > 1) {
        extraGap = free / (kids.length - 1) + gap;
      } else if (justify === 'space-around' && kids.length > 0) {
        const slot = free / kids.length;
        cursor += slot / 2;
        extraGap = slot + gap;
      } else if (justify === 'space-evenly' && kids.length > 0) {
        const slot = free / (kids.length + 1);
        cursor += slot;
        extraGap = slot + gap;
      }
    }

    for (const { node: kid, size, grow } of kidData) {
      // 主軸サイズの決定
      const kidMainBase = row ? size.w : size.h;
      const kidMain = grow > 0 ? remaining * (grow / totalGrow) : kidMainBase;

      // 交差軸サイズの決定
      const crossAvail = row ? innerH : innerW;
      const kidCrossIntrinsic = row ? size.h : size.w;
      // 交差軸に明示 px 指定があれば尊重し、なければ stretch/align に従う
      const hasExplicitCross = row
        ? typeof kid.style.height === 'number'
        : typeof kid.style.width === 'number';
      let kidCross: number;
      let crossPos = row ? innerY : innerX;

      if (align === 'stretch' && !hasExplicitCross) {
        kidCross = crossAvail;
      } else {
        kidCross = kidCrossIntrinsic;
        if (align === 'center') crossPos += (crossAvail - kidCross) / 2;
        else if (align === 'flex-end') crossPos += crossAvail - kidCross;
      }

      if (row) {
        this.place(kid, cursor, crossPos, kidMain, kidCross);
        cursor += kidMain + extraGap;
      } else {
        this.place(kid, crossPos, cursor, kidCross, kidMain);
        cursor += kidMain + extraGap;
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
    // ルートは常にキャンバス全体を占める
    this.place(rootNode, 0, 0, canvas.width, canvas.height);
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

  // --- ヒットテスト ---

  private readonly onClick = (e: MouseEvent): void => {
    const rect = this.canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    const hit = this.hitTest(px, py);
    if (hit !== null) {
      this.eventQueue.push(0 /* click */, hit);
    }
  };

  private hitTest(px: number, py: number): number | null {
    let found: number | null = null;
    const visit = (node: Node): void => {
      const r = node.rect;
      if (!r || node.style.display === 'none') return;
      if (px >= r.x && px <= r.x + r.w && py >= r.y && py <= r.y + r.h) {
        found = node.id;
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
