import type { ElementKind } from '@tsubame/renderer-protocol';
import { elementKindDefaultCursor } from '@tsubame/renderer-protocol';

type StyleDefaults = Partial<Record<keyof CSSStyleDeclaration, string>>;

interface ElementDomSpec {
  readonly tagName: keyof HTMLElementTagNameMap;
  readonly attributes?: Readonly<Record<string, string>>;
  readonly style?: StyleDefaults;
}

/** RN Web 現行方式: 全 kind に stacking ベースを付与（ADR-0006） */
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
  // A multi-line text-input materialises as a `<textarea>` so the browser's
  // native Enter inserts a newline at the caret; single-line uses `<input>`
  // which submits (#362). The text-input baseline styles apply to both.
  const useTextarea = kind === 'text-input' && multiline;
  const el = doc.createElement(useTextarea ? 'textarea' : spec.tagName);

  applyDefaults(el, BASE_STYLE);
  if (spec.style !== undefined) applyDefaults(el, spec.style);
  // UA default cursor per element-kind from the spec single source (ADR-0105),
  // so DOM and Canvas (Hayate core `resolve_cursor`) show the same cursor and the
  // mapping is not re-declared per renderer. An explicit `cursor` style still wins.
  const defaultCursor = elementKindDefaultCursor(kind);
  if (defaultCursor !== undefined) {
    el.style.cursor = defaultCursor;
  }
  for (const [name, value] of Object.entries(spec.attributes ?? {})) {
    // `<textarea>` has no `type` attribute — skip it for the multiline variant.
    if (useTextarea && name === 'type') continue;
    el.setAttribute(name, value);
  }

  return el;
}
