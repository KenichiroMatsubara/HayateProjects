import type { ElementKind } from '@tsubame/renderer-protocol';
import type {
  HayateEvent,
  HayateStyleProp,
  HayateStylePropKind,
  HayateWasm,
} from '@tsubame/renderer-canvas';

/**
 * デモ用の Hayate WASM スタンドイン。
 *
 * WIT element-layer の TypeScript バインディング（HayateWasm）を JS で実装する。
 * 隠し DOM ツリーにスタイルを CSS として適用し、getBoundingClientRect() で
 * 確定座標を取得して Canvas 2D に描く。レイアウト計算はブラウザの CSS エンジンに委譲。
 *
 * ADR-0047 準拠: color は隠し DOM の CSS カスケードが継承を解決する。
 * getComputedStyle(el).color で確定済みの継承色を取得して Canvas に描く。
 * element_unset_style() はインラインスタイルを削除してカスケードに委ねる。
 */
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
  private readonly eventQueue: HayateEvent[] = [];
  private hoveredId: number | null = null;
  private focusedId: number | null = null;

  constructor(private readonly canvas: HTMLCanvasElement) {
    const ctx = canvas.getContext('2d');
    if (ctx === null) throw new Error('2D context を取得できません');
    this.ctx = ctx;

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

  // ─── HayateWasm: 要素ツリー管理 ───────────────────────────────────────────

  element_create(id: number, kind: ElementKind): void {
    this.nodeKind.set(id, kind);
    const el = document.createElement('div');
    el.dataset['id'] = String(id);
    el.style.boxSizing = 'border-box';
    this.domNodes.set(id, el);
  }

  set_root(id: number): void {
    this.root = id;
    const rootEl = this.domNodes.get(id);
    if (rootEl) this.container.replaceChildren(rootEl);
  }

  element_append_child(parent: number, child: number): void {
    const parentEl = this.domNodes.get(parent);
    const childEl  = this.domNodes.get(child);
    if (parentEl && childEl) parentEl.appendChild(childEl);
  }

  element_insert_before(parent: number, child: number, before: number): void {
    const parentEl = this.domNodes.get(parent);
    const childEl  = this.domNodes.get(child);
    const refEl    = this.domNodes.get(before);
    if (parentEl && childEl) parentEl.insertBefore(childEl, refEl ?? null);
  }

  element_remove(id: number): void {
    const el = this.domNodes.get(id);
    el?.parentNode?.removeChild(el);
  }

  element_set_style(id: number, props: HayateStyleProp[]): void {
    const el = this.domNodes.get(id);
    if (!el) return;
    for (const prop of props) {
      if ('background-color' in prop) {
        el.style.backgroundColor = this.colorCss(prop['background-color']);
      } else if ('color' in prop) {
        el.style.color = this.colorCss(prop.color);
      } else if ('border-color' in prop) {
        el.style.borderColor = this.colorCss(prop['border-color']);
      } else if ('opacity' in prop) {
        el.style.opacity = String(prop.opacity);
      } else if ('border-radius' in prop) {
        el.style.borderRadius = `${prop['border-radius']}px`;
      } else if ('border-width' in prop) {
        const v = prop['border-width'];
        el.style.borderWidth = `${v}px`;
        el.style.borderStyle = v > 0 ? 'solid' : 'none';
      } else if ('width' in prop)         { el.style.width          = this.dimCss(prop.width); }
      else if ('height' in prop)          { el.style.height         = this.dimCss(prop.height); }
      else if ('min-width' in prop)       { el.style.minWidth       = this.dimCss(prop['min-width']); }
      else if ('min-height' in prop)      { el.style.minHeight      = this.dimCss(prop['min-height']); }
      else if ('max-width' in prop)       { el.style.maxWidth       = this.dimCss(prop['max-width']); }
      else if ('max-height' in prop)      { el.style.maxHeight      = this.dimCss(prop['max-height']); }
      else if ('display' in prop)         { el.style.display        = prop.display; }
      else if ('flex-direction' in prop)  { el.style.flexDirection  = prop['flex-direction']; }
      else if ('align-items' in prop)     { el.style.alignItems     = prop['align-items']; }
      else if ('justify-content' in prop) { el.style.justifyContent = prop['justify-content']; }
      else if ('gap' in prop)             { el.style.gap            = this.dimCss(prop.gap); }
      else if ('padding' in prop)         { el.style.padding        = this.dimCss(prop.padding); }
      else if ('padding-top' in prop)     { el.style.paddingTop     = this.dimCss(prop['padding-top']); }
      else if ('padding-right' in prop)   { el.style.paddingRight   = this.dimCss(prop['padding-right']); }
      else if ('padding-bottom' in prop)  { el.style.paddingBottom  = this.dimCss(prop['padding-bottom']); }
      else if ('padding-left' in prop)    { el.style.paddingLeft    = this.dimCss(prop['padding-left']); }
      else if ('margin' in prop)          { el.style.margin         = this.dimCss(prop.margin); }
      else if ('margin-top' in prop)      { el.style.marginTop      = this.dimCss(prop['margin-top']); }
      else if ('margin-right' in prop)    { el.style.marginRight    = this.dimCss(prop['margin-right']); }
      else if ('margin-bottom' in prop)   { el.style.marginBottom   = this.dimCss(prop['margin-bottom']); }
      else if ('margin-left' in prop)     { el.style.marginLeft     = this.dimCss(prop['margin-left']); }
      else if ('font-size' in prop)       { el.style.fontSize       = `${prop['font-size']}px`; }
      else if ('font-family' in prop)     { el.style.fontFamily     = prop['font-family']; }
      else if ('z-index' in prop)         { el.style.zIndex         = String(prop['z-index']); }
      else if ('flex-grow' in prop)       { el.style.flexGrow       = String(prop['flex-grow']); }
    }
  }

  /**
   * 継承対象プロパティのリセット（ADR-0047）。
   * インラインスタイルを削除することで、ブラウザの CSS カスケード（= 親からの継承）に委ねる。
   */
  element_unset_style(id: number, kinds: HayateStylePropKind[]): void {
    const el = this.domNodes.get(id);
    if (!el) return;
    for (const kind of kinds) {
      el.style.removeProperty(kind);
    }
  }

  element_set_text(id: number, text: string): void {
    this.nodeText.set(id, text);
    const el = this.domNodes.get(id);
    if (el) el.textContent = text;
  }

  // ─── HayateWasm: フレームライフサイクル ──────────────────────────────────

  on_resize(width: number, height: number): void {
    this.canvas.width  = width;
    this.canvas.height = height;
    this.canvas.style.width  = `${width}px`;
    this.canvas.style.height = `${height}px`;
    this.container.style.left   = `-${width}px`;
    this.container.style.width  = `${width}px`;
    this.container.style.height = `${height}px`;
  }

  render(_timestampMs: number): void {
    const { ctx, canvas } = this;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    if (this.root === null) return;
    this.nodeRects.clear();
    const origin = this.container.getBoundingClientRect();
    this.drawNode(this.root, origin);
  }

  poll_events(): HayateEvent[] {
    return this.eventQueue.splice(0);
  }

  // ─── CSS 変換ヘルパー ─────────────────────────────────────────────────────

  private colorCss({ r, g, b, a }: { r: number; g: number; b: number; a: number }): string {
    return `rgba(${Math.round(r * 255)},${Math.round(g * 255)},${Math.round(b * 255)},${a})`;
  }

  private dimCss({ value, unit }: { value: number; unit: string }): string {
    if (unit === 'percent') return `${value}%`;
    if (unit === 'auto')    return 'auto';
    if (unit === 'fr')      return `${value}fr`;
    return `${value}px`;
  }

  // ─── 描画（getBoundingClientRect → Canvas 2D）──────────────────────────────

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

    const opacity = parseFloat(el.style.opacity);
    if (!isNaN(opacity)) ctx.globalAlpha *= opacity;

    if (el.style.backgroundColor) {
      const radius = parseFloat(el.style.borderRadius) || 0;
      this.roundRect(x, y, w, h, radius);
      ctx.fillStyle = el.style.backgroundColor;
      ctx.fill();
    }

    const kind = this.nodeKind.get(id) ?? 'view';
    const text = this.nodeText.get(id) ?? '';
    if ((kind === 'text' || kind === 'button') && el.children.length === 0 && text) {
      const fontSize = parseFloat(el.style.fontSize) || 16;
      ctx.font         = `${fontSize}px system-ui, sans-serif`;
      // ADR-0047: getComputedStyle で CSS カスケードを通じた継承色を取得する。
      ctx.fillStyle    = getComputedStyle(el).color;
      ctx.textBaseline = 'middle';
      ctx.textAlign    = 'center';
      ctx.fillText(text, x + w / 2, y + h / 2);
    }

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
      this.eventQueue.push({ type: 'click', target: hit, x: px, y: py });
    }
  };

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
      this.eventQueue.push({ type: 'hover-leave', target: this.hoveredId });
    }
    this.hoveredId = nextId;
    if (nextId !== null) {
      this.eventQueue.push({ type: 'hover-enter', target: nextId });
    }
  }

  private syncFocus(nextId: number | null): void {
    if (nextId === this.focusedId) return;
    if (this.focusedId !== null) {
      this.eventQueue.push({ type: 'blur', target: this.focusedId });
    }
    this.focusedId = nextId;
    if (nextId !== null) {
      this.eventQueue.push({ type: 'focus', target: nextId });
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
