// 自動生成ファイル（Tsubame/proto/generator） — 手動で編集しないこと
// 生成元: @hayate/protocol-spec

import type { HayateDimension, HayateGridPlacement, HayateShadow } from '../style-primitives.js';

export type Display = 'flex' | 'grid' | 'block' | 'none';
export type FlexDirection = 'row' | 'column' | 'row-reverse' | 'column-reverse';
export type FlexWrap = 'nowrap' | 'wrap' | 'wrap-reverse';
export type AlignItems = 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
export type AlignSelf = 'auto' | 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'baseline';
export type AlignContent = 'flex-start' | 'flex-end' | 'center' | 'stretch' | 'space-between' | 'space-around' | 'space-evenly';
export type JustifyContent = 'flex-start' | 'flex-end' | 'center' | 'space-between' | 'space-around' | 'space-evenly';
export type FontStyle = 'normal' | 'italic' | 'oblique';
export type TextDecoration = 'none' | 'underline' | 'line-through';
export type BorderStyle = 'none' | 'solid' | 'dashed';
export type Cursor = 'default' | 'pointer' | 'text' | 'crosshair' | 'not-allowed' | 'grab' | 'grabbing';
export type Overflow = 'visible' | 'hidden';
export type TextOverflow = 'clip' | 'ellipsis';
export type Position = 'relative' | 'absolute';
export type TransitionTiming = 'ease' | 'linear' | 'ease-in' | 'ease-out' | 'ease-in-out';
export type BoxSizing = 'border-box' | 'content-box';
export type GridAutoFlow = 'row' | 'column' | 'row-dense' | 'column-dense';
export type JustifyItems = 'start' | 'end' | 'center' | 'stretch';
export type JustifySelf = 'auto' | 'start' | 'end' | 'center' | 'stretch';

export interface HayateStyle {
  backgroundColor: string;
  opacity: number;
  borderRadius: number;
  borderWidth: number;
  borderColor: string;
  width: HayateDimension;
  height: HayateDimension;
  minWidth: HayateDimension;
  minHeight: HayateDimension;
  maxWidth: HayateDimension;
  maxHeight: HayateDimension;
  display: Display;
  flexDirection: FlexDirection;
  alignItems: AlignItems;
  justifyContent: JustifyContent;
  gap: HayateDimension;
  padding: HayateDimension;
  paddingTop: HayateDimension;
  paddingRight: HayateDimension;
  paddingBottom: HayateDimension;
  paddingLeft: HayateDimension;
  margin: HayateDimension;
  marginTop: HayateDimension;
  marginRight: HayateDimension;
  marginBottom: HayateDimension;
  marginLeft: HayateDimension;
  fontSize: number;
  color: string;
  zIndex: number;
  fontFamily: string;
  flexGrow: number;
  fontWeight: number;
  fontStyle: FontStyle;
  textDecoration: TextDecoration;
  defaultColor: string;
  defaultFontFamily: string;
  defaultFontSize: number;
  defaultFontWeight: number;
  gridTemplateColumns: HayateDimension[];
  gridTemplateRows: HayateDimension[];
  flexShrink: number;
  flexBasis: HayateDimension;
  alignSelf: AlignSelf;
  alignContent: AlignContent;
  flexWrap: FlexWrap;
  borderStyle: BorderStyle;
  cursor: Cursor;
  position: Position;
  top: HayateDimension;
  left: HayateDimension;
  right: HayateDimension;
  bottom: HayateDimension;
  overflow: Overflow;
  maxLines: number;
  textOverflow: TextOverflow;
  transitionDuration: number;
  transitionTiming: TransitionTiming;
  boxShadow: HayateShadow[];
  aspectRatio: number;
  boxSizing: BoxSizing;
  gridAutoRows: HayateDimension[];
  gridAutoColumns: HayateDimension[];
  gridAutoFlow: GridAutoFlow;
  gridColumn: HayateGridPlacement;
  justifyItems: JustifyItems;
  justifySelf: JustifySelf;
  gridRow: HayateGridPlacement;
}

/**
 * `IRenderer.setStyle` のパッチ意味論。
 *
 * - 存在するプロパティは以前の値を上書きする。
 * - 存在しないプロパティは以前の値を保持する。
 * - `null` は、対象レンダラーがリセットに対応している場合にプロパティをリセットする。
 */
export type StylePatch = {
  [K in keyof HayateStyle]?: HayateStyle[K] | null;
};
