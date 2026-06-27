import type { IRenderer } from '@tsubame/renderer-protocol';
import { applyElementProp } from '@tsubame/renderer-protocol';
import type { TsubameInstance } from './instance.js';

/** React が予約する／構造に関与する prop（差分ループでスキップする）。 */
const STRUCTURAL_PROPS = new Set(['children', 'ref', 'key']);

/**
 * 1 つの prop の変化を `IRenderer` へ適用する。style チャンネル・event 語彙・閉じた
 * 要素プロパティの dispatch は `tsubame-solid` と共通の `applyElementProp` seam に委譲する
 * （ADR-0010）。差分は React 側で diff 済みのため、ここでは個々の prop を冪等に書き戻す。
 */
export function applyProp(
  renderer: IRenderer,
  instance: TsubameInstance,
  name: string,
  value: unknown,
): void {
  applyElementProp(renderer, instance, name, value);
}

/** mount 時の初期 prop を一括適用する。 */
export function applyInitialProps(
  renderer: IRenderer,
  instance: TsubameInstance,
  props: Record<string, unknown>,
): void {
  for (const [name, value] of Object.entries(props)) {
    applyProp(renderer, instance, name, value);
  }
}

/**
 * 更新コミット時に prev → next の差分を適用する。next に無い prop は消去
 *（イベントは解除、style は空パッチ）し、変化した prop だけ書き戻す。
 */
export function applyPropUpdates(
  renderer: IRenderer,
  instance: TsubameInstance,
  prevProps: Record<string, unknown>,
  nextProps: Record<string, unknown>,
): void {
  for (const name of Object.keys(prevProps)) {
    if (STRUCTURAL_PROPS.has(name)) continue;
    if (!(name in nextProps)) {
      applyProp(renderer, instance, name, undefined);
    }
  }
  for (const [name, value] of Object.entries(nextProps)) {
    if (STRUCTURAL_PROPS.has(name)) continue;
    if (!Object.is(prevProps[name], value)) {
      applyProp(renderer, instance, name, value);
    }
  }
}
