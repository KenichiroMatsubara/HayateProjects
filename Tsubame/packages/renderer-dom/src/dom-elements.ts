import type { ElementKind } from '@tsubame/renderer-protocol';

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
  color: 'inherit',
  cursor: 'inherit',
  font: 'inherit',
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
      cursor: 'pointer',
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
): HTMLElement {
  const spec = ELEMENT_SPECS[kind];
  const el = doc.createElement(spec.tagName);

  applyDefaults(el, BASE_STYLE);
  if (spec.style !== undefined) applyDefaults(el, spec.style);
  for (const [name, value] of Object.entries(spec.attributes ?? {})) {
    el.setAttribute(name, value);
  }

  return el;
}
