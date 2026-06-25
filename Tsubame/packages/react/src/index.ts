export { renderTsubame, createTsubameRoot } from './mount.js';
export type { TsubameRoot } from './mount.js';

export { createReconciler } from './host-config.js';
export type { TsubameContainer, TsubameReconciler } from './host-config.js';

export type { TsubameInstance, TsubameTextInstance } from './instance.js';
export type { TsubameProps, StyleVariant, TsubameIntrinsicElements } from './jsx.js';

// JSX の Element 語彙は `jsxImportSource: "@tsubame/react"` 経由で
// `./jsx-runtime` の JSX 名前空間から解決する（副作用 import は不要）。
