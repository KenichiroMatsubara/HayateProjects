import type {
  DrawProperty,
  HayateCssStyle,
  EventHandler,
  UserSelect,
  ViewportCondition,
} from '@torimi/tsubame-renderer-protocol';
import type { ReactNode, Key } from 'react';

/**
 * ビューポート条件付きスタイル変種（ADR-0081）。DOM Renderer では本物の
 * `@media (min-width: …)` ルールへ、Canvas Renderer では viewport 評価へ落ちる。
 */
export interface StyleVariant {
  condition: ViewportCondition;
  style: HayateCssStyle;
}

/**
 * Tsubame の Element 語彙に共通の JSX prop。`tsubame-solid` の `TsubameProps` と対称。
 */
export interface TsubameProps {
  style?: HayateCssStyle;
  /** ビューポート条件付きスタイル（ADR-0081）。宣言順に適用される。 */
  styleVariants?: readonly StyleVariant[];
  onClick?: EventHandler;
  onInput?: EventHandler;
  onKeyDown?: EventHandler;
  onFocus?: EventHandler;
  onBlur?: EventHandler;
  children?: ReactNode;
  key?: Key;
}

/**
 * Tsubame の Element 語彙（`view` / `text` / `button` / `text-input` /
 * `scroll-view` / `image`）に対する JSX intrinsic 型。
 *
 * これらは独自の `jsx-runtime` が export する `JSX` 名前空間で使う（{@link ./jsx-runtime.js}）。
 * `@types/react` の `JSX.IntrinsicElements` は `view` / `text` / `image`（SVG）や
 * `button`（HTML）を既に別の型で宣言しているため module augmentation では上書きできない。
 * そこで `jsxImportSource: "@torimi/tsubame-react"` で専用 JSX 名前空間に差し替える（標準
 * `jsx: "react-jsx"`、compile プラグイン不要。ADR-0010）。
 */
export interface TsubameIntrinsicElements {
  /** `draw`: 命令的 2D 描画の painter（#730 / ADR-0141）。view 限定（carriesDraw）。 */
  view: TsubameProps & { 'user-select'?: UserSelect; draw?: DrawProperty | null };
  text: TsubameProps & { 'user-select'?: UserSelect };
  image: TsubameProps & { src?: string; 'user-select'?: UserSelect };
  button: TsubameProps & { 'user-select'?: UserSelect };
  'text-input': TsubameProps & {
    value?: string;
    placeholder?: string;
    disabled?: boolean;
    multiline?: boolean;
  };
  'scroll-view': TsubameProps & { 'user-select'?: UserSelect };
}
