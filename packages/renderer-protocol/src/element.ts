/**
 * Renderer Protocol 全体で element を識別する opaque 型。
 *
 * JS 実行時は number のままでゼロオーバーヘッド。Canvas Renderer の
 * `ops: Float64Array` への格納時は `id as number` で unwrap する。
 * id の採番は各 Renderer 実装（JS 側モノトニックカウンター）の責務であり、
 * Protocol は採番方法を規定しない。
 */
export type ElementId = number & { __brand: 'ElementId' };

/**
 * Tsubame が扱う UI の構成単位の種別。Hayate の Element Layer に対応し、
 * React Native 語彙を採用する。HTML タグ名（div / span 等）は使わない。
 *
 * DOM Renderer は各 kind を対応する HTML 要素へマッピングする
 * （view→div / text→span / image→img / button→button /
 *  text-input→input / scroll-view→overflow:auto な div）。
 */
export type ElementKind =
  | 'view'
  | 'text'
  | 'image'
  | 'button'
  | 'text-input'
  | 'scroll-view';

/**
 * 素の number を {@link ElementId} としてブランド付けするヘルパー。
 * Renderer 実装内で採番した number を Protocol 型に持ち上げる用途。
 */
export const asElementId = (n: number): ElementId => n as ElementId;
