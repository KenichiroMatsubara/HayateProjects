import type * as React from 'react';
import type { TsubameIntrinsicElements } from './jsx.js';

// 実体は React 標準の automatic runtime を再 export する（変換は React に委ねる）。
export { Fragment, jsx, jsxs } from 'react/jsx-runtime';

/**
 * `jsxImportSource: "@torimi/tsubame-react"` のときに TypeScript が参照する JSX 名前空間。
 *
 * `IntrinsicElements` だけを Tsubame の Element 語彙へ差し替え、それ以外の構造的な
 * JSX 型は React のものを再利用する。これにより `@types/react` の SVG/HTML intrinsic
 * との衝突なしに `<view>` / `<text>` 等へ独自の prop 型を与えられる（ADR-0010）。
 */
export namespace JSX {
  export type ElementType = React.JSX.ElementType;
  export interface Element extends React.JSX.Element {}
  export interface ElementClass extends React.JSX.ElementClass {}
  export interface ElementAttributesProperty extends React.JSX.ElementAttributesProperty {}
  export interface ElementChildrenAttribute extends React.JSX.ElementChildrenAttribute {}
  export type LibraryManagedAttributes<C, P> = React.JSX.LibraryManagedAttributes<C, P>;
  export interface IntrinsicAttributes extends React.JSX.IntrinsicAttributes {}
  export interface IntrinsicClassAttributes<T> extends React.JSX.IntrinsicClassAttributes<T> {}
  export interface IntrinsicElements extends TsubameIntrinsicElements {}
}
