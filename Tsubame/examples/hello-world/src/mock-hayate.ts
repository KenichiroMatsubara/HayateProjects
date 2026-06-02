import {
  OP,
  OP_SLOTS,
  TAG,
  type HayateWasm,
} from '@tsubame/renderer-canvas';

/**
 * デモ用の Hayate WASM スタンドイン。
 *
 * 実 Hayate（Rust → Taffy → Vello → WebGPU）と同じ apply_mutations 契約を JS で受け取り、
 * 隠し DOM ツリーにスタイルをそのまま CSS として適用する。レイアウト計算はブラウザの
 * CSS エンジンに委譲し、getBoundingClientRect() で確定座標を取得して Canvas 2D に描く。
 *
 * measure() / place() による JS 内 Flexbox 再実装は行わない。
 * CSS が増えても MockHayate は変更不要。
 *
 * ADR-0047 準拠: color は隠し DOM の CSS カスケードが継承を解決する。
 * getComputedStyle(el).color で確定済みの継承色を取得して Canvas に描く。
 * element_unset_style() はインラインスタイルを削除してカスケードに委ねる。
 */

const KIND_NAME = ['view', 'text', 'image', 'button', 'text-input', 'scroll-view'];
const textDecoder = new TextDecoder();

const DISPLAY_CSS = ['flex', 'grid', 'block', 'none'] as const;
const FLEX_DIRECTION_CSS = ['row', 'column', 'row-reverse', 'column-reverse'] as const;
const ALIGN_ITEMS_CSS  = ['flex-start', 'flex-end', 'center', 'stretch', 'baseline'] as const;
const JUSTIFY_CSS = [
  'flex-start', 'flex-end', 'center', 'space-between', 'space-around', 'space-evenly',
] as const;
const EVENT = {
  click: 0,
  focus: 1,
  blur: 2,
  'hover-enter': 10,
  'hover-leave': 11,
} as const;

export class MockHayate implements HayateWasm {
  private readonly ctx: CanvasRenderingContext2D;
  /** ブラウザの CSS エンジンでレイアウトを計算する隠し DOM コンテナ */
  private readonly container: HTMLDivElement;
  private readonly domNodes = new Map<number, HTMLDivElement>();
  private readonly nodeKind  = new Map<number, string>();
  private readonly nodeText  = new Map<number, string>();
  /** render() で確定した canvas 座標系の矩形（ヒットテスト用） */
  private readonly nodeRects = new Map<number, { x: number; y: number; w: number; h: number }>();
  private root: number | null = null;
  private readonly eventQueue: number[] = [];
  private hoveredId: number | null = null;
  private focusedId: number | null = null;

  constructor(private readonly canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext('2d');
    if (ctx === null) throw new Error('2D context を取得できません');
    this.ctx = ctx;

    // visibility:hidden で画面外に配置。display:none だとレイアウトが計算されない。
    this.container = document.createElement('div');
    this.container.style.cssText =
      `position:fixed;top:0;left:-${canvas.width}px;` +
      `width:${canvas.width}px;height:${canvas.height}px;` +
      `visibility:hidden;pointer-events:none;overflow:hidden;` +
      `font-family:system-ui,sans-serif;box-sizing:border-box;`;
    document.body.appendChild(this.container);

    if (canvas.tabIndex < 0) canvas.tabIndex = 0;
    canvas.addEventListener('click', this.onClick);
    canvas.addEventListener('mousemove', this.onMouseMove);
    canvas.addEventListener('mouseleave', this.onMouseLeave);
    canvas.addEventListener('blur', this.onCanvasBlur);
  }

  dispose(): void {
    this.canvas.removeEventListener('click', this.onClick);
    this.canvas.removeEventListener('mousemove', this.onMouseMove);
    this.canvas.removeEventListener('mouseleave', this.onMouseLeave);
    this.canvas.removeEventListener('blur', this.onCanvasBlur);
    document.body.removeChild(this.container);
  }

  resize(width: number, height: number): void {
    this.canvas.width  = width;
    this.canvas.height = height;
    this.canvas.style.width  = `${width}px`;
    this.canvas.style.height = `${height}px`;
    // コンテナも同サイズに更新（% 幅の基準になる）
    this.container.style.left   = `-${width}px`;
    this.container.style.width  = `${width}px`;
    this.container.style.height = `${height}px`;
    this.render();
  }

  apply_mutations(ops: Float64Array, styles: Float32Array): void {
    let i = 0;
    while (i < ops.length) {
      const op    = ops[i]!;
      const slots = OP_SLOTS[op];
      if (slots === undefined) break;

      switch (op) {
        case OP.CREATE: {
          const id   = ops[i + 1]!;
          const kind = KIND_NAME[ops[i + 2]!] ?? 'view';
          this.nodeKind.set(id, kind);
          const el = document.createElement('div');
          el.dataset['id']     = String(id);
          el.style.boxSizing   = 'border-box';
          this.domNodes.set(id, el);
          break;
        }
        case OP.SET_ROOT: {
          this.root = ops[i + 1]!;
          const rootEl = this.domNodes.get(this.root);
          if (rootEl) this.container.replaceChildren(rootEl);
          break;
        }
        case OP.APPEND_CHILD: {
          const parent = this.domNodes.get(ops[i + 1]!);
          const child  = this.domNodes.get(ops[i + 2]!);
          if (parent && child) parent.appendChild(child);
          break;
        }
        case OP.INSERT_BEFORE: {
          const parent = this.domNodes.get(ops[i + 1]!);
          const child  = this.domNodes.get(ops[i + 2]!);
          const ref    = this.domNodes.get(ops[i + 3]!);
          if (parent && child) parent.insertBefore(child, ref ?? null);
          break;
        }
        case OP.REMOVE: {
          const el = this.domNodes.get(ops[i + 1]!);
          el?.parentNode?.removeChild(el);
          break;
        }
        case OP.SET_STYLE:
          this.applyStyle(ops[i + 1]!, styles, ops[i + 2]!, ops[i + 3]!);
          break;
        default:
          break;
      }
      i += slots;
    }
    this.render();
  }

  element_set_text(id: number, text: string): void {
    this.nodeText.set(id, text);
    // 隠し DOM にテキストを反映してブラウザのテキスト幅計算に使わせる
    const el = this.domNodes.get(id);
    if (el) el.textContent = text;
    this.render();
  }

  /**
   * 継承対象プロパティのリセット（ADR-0047）。
   * kinds: 0=Color, 1=FontSize, 2=FontFamily
   * インラインスタイルを削除することで、ブラウザの CSS カスケード（= 親からの継承）に委ねる。
   */
  element_unset_style(id: number, kinds: Uint32Array): void {
    const el = this.domNodes.get(id);
    if (!el) return;
    for (const kind of kinds) {
      if (kind === 0) el.style.removeProperty('color');
      if (kind === 1) el.style.removeProperty('font-size');
      if (kind === 2) el.style.removeProperty('font-family');
    }
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

  // ─── スタイル適用（ops バッファのタグを CSS プロパティに変換）──────────────

  private applyStyle(id: number, styles: Float32Array, offset: number, len: number): void {
    const el = this.domNodes.get(id);
    if (!el) return;
    let i = offset;
    const end = offset + len;
    while (i < end) {
      const tag = styles[i++]!;
      if (tag === TAG.COLOR || tag === TAG.BACKGROUND_COLOR || tag === TAG.BORDER_COLOR) {
        const r = Math.round(styles[i]!     * 255);
        const g = Math.round(styles[i + 1]! * 255);
        const b = Math.round(styles[i + 2]! * 255);
        const a = styles[i + 3]!;
        i += 4;
        const css = `rgba(${r},${g},${b},${a})`;
        if (tag === TAG.COLOR) el.style.color = css;
        else if (tag === TAG.BACKGROUND_COLOR) el.style.backgroundColor = css;
        else el.style.borderColor = css;
      } else if (
        tag === TAG.WIDTH ||
        tag === TAG.HEIGHT ||
        tag === TAG.MIN_WIDTH ||
        tag === TAG.MIN_HEIGHT ||
        tag === TAG.MAX_WIDTH ||
        tag === TAG.MAX_HEIGHT ||
        tag === TAG.GAP ||
        tag === TAG.PADDING ||
        tag === TAG.PADDING_TOP ||
        tag === TAG.PADDING_RIGHT ||
        tag === TAG.PADDING_BOTTOM ||
        tag === TAG.PADDING_LEFT ||
        tag === TAG.MARGIN ||
        tag === TAG.MARGIN_TOP ||
        tag === TAG.MARGIN_RIGHT ||
        tag === TAG.MARGIN_BOTTOM ||
        tag === TAG.MARGIN_LEFT
      ) {
        const v    = styles[i++]!;
        const unit = styles[i++]!;  // 0=px, 1=percent, 2=auto, 3=fr
        const css  = unit === 1 ? `${v}%` : unit === 2 ? 'auto' : unit === 3 ? `${v}fr` : `${v}px`;
        if (tag === TAG.WIDTH)            el.style.width = css;
        else if (tag === TAG.HEIGHT)      el.style.height = css;
        else if (tag === TAG.MIN_WIDTH)   el.style.minWidth = css;
        else if (tag === TAG.MIN_HEIGHT)  el.style.minHeight = css;
        else if (tag === TAG.MAX_WIDTH)   el.style.maxWidth = css;
        else if (tag === TAG.MAX_HEIGHT)  el.style.maxHeight = css;
        else if (tag === TAG.GAP)         el.style.gap = css;
        else if (tag === TAG.PADDING)        el.style.padding = css;
        else if (tag === TAG.PADDING_TOP)    el.style.paddingTop = css;
        else if (tag === TAG.PADDING_RIGHT)  el.style.paddingRight = css;
        else if (tag === TAG.PADDING_BOTTOM) el.style.paddingBottom = css;
        else if (tag === TAG.PADDING_LEFT)   el.style.paddingLeft = css;
        else if (tag === TAG.MARGIN)         el.style.margin = css;
        else if (tag === TAG.MARGIN_TOP)     el.style.marginTop = css;
        else if (tag === TAG.MARGIN_RIGHT)   el.style.marginRight = css;
        else if (tag === TAG.MARGIN_BOTTOM)  el.style.marginBottom = css;
        else if (tag === TAG.MARGIN_LEFT)    el.style.marginLeft = css;
      } else if (tag === TAG.FONT_FAMILY) {
        const len = styles[i++]!;
        const bytes = Uint8Array.from(styles.slice(i, i + len));
        i += len;
        el.style.fontFamily = textDecoder.decode(bytes);
      } else {
        const v = styles[i++]!;
        switch (tag) {
          case TAG.DISPLAY:
            el.style.display = DISPLAY_CSS[v] ?? 'flex';
            break;
          case TAG.FLEX_DIRECTION:
            el.style.flexDirection = FLEX_DIRECTION_CSS[v] ?? 'row';
            break;
          case TAG.ALIGN_ITEMS:
            el.style.alignItems = ALIGN_ITEMS_CSS[v] ?? 'stretch';
            break;
          case TAG.JUSTIFY_CONTENT:
            el.style.justifyContent = JUSTIFY_CSS[v] ?? 'flex-start';
            break;
          case TAG.BORDER_RADIUS:
            el.style.borderRadius = `${v}px`;
            break;
          case TAG.OPACITY:
            el.style.opacity = String(v);
            break;
          case TAG.FONT_SIZE:
            el.style.fontSize = `${v}px`;
            break;
          case TAG.BORDER_WIDTH:
            el.style.borderWidth = `${v}px`;
            el.style.borderStyle = v > 0 ? 'solid' : 'none';
            break;
          case TAG.Z_INDEX:
            el.style.zIndex = String(v);
            break;
          case TAG.FLEX_GROW:
            el.style.flexGrow = String(v);
            break;
          case TAG.FONT_WEIGHT:
            el.style.fontWeight = String(v);
            break;
          default:
            break;
        }
      }
    }
  }

  // ─── 描画（getBoundingClientRect → Canvas 2D）──────────────────────────────

  private render(): void {
    const { ctx, canvas } = this;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (this.root === null) return;
    this.nodeRects.clear();
    // getBoundingClientRect() はレイアウトを同期的に確定させる
    const origin = this.container.getBoundingClientRect();
    this.drawNode(this.root, origin);
  }

  private drawNode(id: number, origin: DOMRect): void {
    const el = this.domNodes.get(id);
    if (!el || el.style.display === 'none') return;

    const r = el.getBoundingClientRect();
    const x = r.left - origin.left;
    const y = r.top  - origin.top;
    const w = r.width;
    const h = r.height;
    this.nodeRects.set(id, { x, y, w, h });

    const { ctx } = this;
    ctx.save();

    // opacity は ctx.save/restore でスタック管理するため累積乗算
    const opacity = parseFloat(el.style.opacity);
    if (!isNaN(opacity)) ctx.globalAlpha *= opacity;

    if (el.style.backgroundColor) {
      const radius = parseFloat(el.style.borderRadius) || 0;
      this.roundRect(x, y, w, h, radius);
      ctx.fillStyle = el.style.backgroundColor;
      ctx.fill();
    }

    // テキスト描画：テキストコンテンツを持つ末端ノード（text / button）
    const kind = this.nodeKind.get(id) ?? 'view';
    const text = this.nodeText.get(id) ?? '';
    if ((kind === 'text' || kind === 'button') && el.children.length === 0 && text) {
      const fontSize = parseFloat(el.style.fontSize) || 16;
      ctx.font          = `${fontSize}px system-ui, sans-serif`;
      // ADR-0047: getComputedStyle で CSS カスケードを通じた継承色を取得する。
      // el.style.color（インライン）が未設定でも、親から継承した色が返る。
      ctx.fillStyle     = getComputedStyle(el).color;
      ctx.textBaseline  = 'middle';
      ctx.textAlign     = 'center';
      ctx.fillText(text, x + w / 2, y + h / 2);
    }

    // 子ノードを深さ優先で描画（この save/restore 内なので opacity が累積する）
    for (const child of el.children) {
      const childId = parseInt((child as HTMLElement).dataset['id'] ?? '');
      if (!isNaN(childId)) this.drawNode(childId, origin);
    }

    ctx.restore();
  }

  private roundRect(x: number, y: number, w: number, h: number, radius: number): void {
    const r = Math.min(radius, w / 2, h / 2);
    const { ctx } = this;
    ctx.beginPath();
    ctx.moveTo(x + r, y);
    ctx.arcTo(x + w, y,     x + w, y + h, r);
    ctx.arcTo(x + w, y + h, x,     y + h, r);
    ctx.arcTo(x,     y + h, x,     y,     r);
    ctx.arcTo(x,     y,     x + w, y,     r);
    ctx.closePath();
  }

  // ─── ヒットテスト ─────────────────────────────────────────────────────────

  private readonly onClick = (e: MouseEvent): void => {
    const rect = this.canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    const hit = this.hitTest(px, py);
    this.syncFocus(hit);
    if (hit !== null) {
      this.canvas.focus();
      this.eventQueue.push(EVENT.click, hit);
    }
  };

  /** 深い子ほど後から上書きされるため、最前面（最深）要素が返る */
  private readonly onMouseMove = (e: MouseEvent): void => {
    const rect = this.canvas.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    this.syncHover(this.hitTest(px, py));
  };

  private readonly onMouseLeave = (): void => {
    this.syncHover(null);
  };

  private readonly onCanvasBlur = (): void => {
    this.syncFocus(null);
  };

  private syncHover(nextId: number | null): void {
    if (nextId === this.hoveredId) return;
    if (this.hoveredId !== null) {
      this.eventQueue.push(EVENT['hover-leave'], this.hoveredId);
    }
    this.hoveredId = nextId;
    if (nextId !== null) {
      this.eventQueue.push(EVENT['hover-enter'], nextId);
    }
  }

  private syncFocus(nextId: number | null): void {
    if (nextId === this.focusedId) return;
    if (this.focusedId !== null) {
      this.eventQueue.push(EVENT.blur, this.focusedId);
    }
    this.focusedId = nextId;
    if (nextId !== null) {
      this.eventQueue.push(EVENT.focus, nextId);
    }
  }

  private hitTest(px: number, py: number): number | null {
    let found: number | null = null;
    for (const [id, r] of this.nodeRects) {
      if (px >= r.x && px < r.x + r.w && py >= r.y && py < r.y + r.h) {
        found = id;
      }
    }
    return found;
  }
}
