import type { ElementKind } from '@tsubame/renderer-protocol';
import { elementKindDefaultCursor } from '@tsubame/renderer-protocol';

type StyleDefaults = Partial<Record<keyof CSSStyleDeclaration, string>>;

interface ElementDomSpec {
  readonly tagName: keyof HTMLElementTagNameMap;
  readonly attributes?: Readonly<Record<string, string>>;
  readonly style?: StyleDefaults;
}

/** 全 kind に stacking のベースを付与（ADR-0006） */
const BASE_STYLE: StyleDefaults = {
  appearance: 'none',
  background: 'none',
  border: '0',
  boxSizing: 'border-box',
  cursor: 'inherit',
  margin: '0',
  outline: 'none',
  padding: '0',
  position: 'relative',
  textAlign: 'inherit',
  webkitAppearance: 'none',
  zIndex: '0',
};

const ELEMENT_SPECS: Record<ElementKind, ElementDomSpec> = {
  view: { tagName: 'div' },
  text: { tagName: 'span' },
  image: { tagName: 'img' },
  button: {
    tagName: 'button',
    style: {
      whiteSpace: 'nowrap',
    },
  },
  'text-input': {
    tagName: 'input',
    attributes: {
      type: 'text',
    },
    style: {
      padding: '0 12px',
    },
  },
  'scroll-view': {
    tagName: 'div',
    style: {
      overflow: 'auto',
      // オーバーレイ式スクロールバー: ガターを確保しないため、スクロール可能な
      // scroll-view でもコンテンツボックス幅をフルに保ち、ガター概念を持たない
      // Canvas と一致する。視覚の正準は DOM のまま（ADR-0102）。古典的なガターを
      // 落とすだけで、スクロール自体は失わない。
      scrollbarWidth: 'none',
    },
  },
};

function applyDefaults(el: HTMLElement, defaults: StyleDefaults): void {
  Object.assign(el.style, defaults);
}

export function createDomElement(
  doc: Document,
  kind: ElementKind,
  multiline = false,
): HTMLElement {
  const spec = ELEMENT_SPECS[kind];
  // 複数行 text-input は `<textarea>` として生成し、ブラウザネイティブの Enter が
  // キャレット位置に改行を挿入するようにする。単一行は送信動作の `<input>` を使う。
  // text-input のベーススタイルは両者に適用する。
  const useTextarea = kind === 'text-input' && multiline;
  const el = doc.createElement(useTextarea ? 'textarea' : spec.tagName);

  applyDefaults(el, BASE_STYLE);
  if (spec.style !== undefined) applyDefaults(el, spec.style);
  // element-kind ごとの UA 既定カーソルを仕様の単一ソースから取得する（ADR-0105）。
  // DOM と Canvas（Hayate core `resolve_cursor`）が同じカーソルを示し、マッピングを
  // レンダラごとに再宣言せずに済む。明示的な `cursor` スタイルがあればそちらが優先。
  const defaultCursor = elementKindDefaultCursor(kind);
  if (defaultCursor !== undefined) {
    el.style.cursor = defaultCursor;
  }
  for (const [name, value] of Object.entries(spec.attributes ?? {})) {
    // `<textarea>` に `type` 属性は無いため、複数行版ではスキップする。
    if (useTextarea && name === 'type') continue;
    el.setAttribute(name, value);
  }

  return el;
}
