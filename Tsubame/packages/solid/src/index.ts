// solid-js/universal のコンパイル済み JSX が import する関数群。
// vite-plugin-solid を `generate: 'universal'` / `moduleName: '@tsubame/solid'`
// で構成すると、これらが呼び出される。
export {
  render,
  effect,
  memo,
  createComponent,
  createElement,
  createTextNode,
  insertNode,
  insert,
  spread,
  setProp,
  mergeProps,
} from './renderer.js';

export { renderTsubame } from './mount.js';
export { setActiveRenderer, activeRenderer } from './active-renderer.js';

export type { TsubameNode, ElementNode, TextNode } from './node.js';
export type { TsubameProps } from './jsx.js';

// グローバル JSX 名前空間の副作用 import。
import './jsx.js';
