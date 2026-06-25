import type {
  EventHandler,
  IRenderer,
  StylePatch,
  ViewportCondition,
} from '@tsubame/renderer-protocol';
import {
  assertKnownElementProperty,
  splitHayateStyle,
  EVENT_PROP,
  REJECTED_EVENT_PROPS,
} from '@tsubame/renderer-protocol';
import type { TsubameInstance } from './instance.js';

/** React が予約する／構造に関与する prop（ホストへは流さない）。 */
const STRUCTURAL_PROPS = new Set(['children', 'ref', 'key']);

/**
 * 1 つの prop の変化を `IRenderer` へ適用する。`tsubame-solid` の `setProperty` と
 * 同じ意味論を持つ（イベント語彙・style チャンネル・閉じた要素プロパティ）。差分は
 * React 側で diff 済みのため、ここでは個々の prop を冪等に書き戻すだけでよい。
 */
export function applyProp(
  renderer: IRenderer,
  instance: TsubameInstance,
  name: string,
  value: unknown,
): void {
  if (STRUCTURAL_PROPS.has(name)) return;

  // text も Hayate element なので style は適用する（ADR-0058）。
  if (name === 'style') {
    const { base, pseudo } = splitHayateStyle((value ?? {}) as Record<string, unknown>);
    renderer.setStyle(instance.id, base as StylePatch);
    for (const [key, block] of Object.entries(pseudo)) {
      if (block !== undefined) {
        renderer.setPseudoStyle(instance.id, key as ':hover' | ':active' | ':focus', block);
      }
    }
    return;
  }

  // ビューポート条件付きスタイル変種（ADR-0081）。
  if (name === 'styleVariants') {
    const variants = (value ?? []) as ReadonlyArray<{
      condition: ViewportCondition;
      style: Record<string, unknown>;
    }>;
    for (const variant of variants) {
      const { base } = splitHayateStyle((variant.style ?? {}) as Record<string, unknown>);
      renderer.setStyleVariant(instance.id, variant.condition, base as StylePatch);
    }
    return;
  }

  // text 要素は style 以外のプロパティを持たない。
  if (instance.kind === 'text') return;

  if (REJECTED_EVENT_PROPS.has(name)) {
    throw new Error(
      `${name} is not supported in tsubame-react. Use ':hover' in style for visual feedback (ADR-0056, ADR-0059).`,
    );
  }

  const eventKind = EVENT_PROP[name];
  if (eventKind !== undefined) {
    instance.listeners.get(name)?.();
    instance.listeners.delete(name);
    if (typeof value === 'function') {
      instance.listeners.set(
        name,
        renderer.addEventListener(instance.id, eventKind, value as EventHandler),
      );
    }
    return;
  }

  assertKnownElementProperty(name);
  renderer.setProperty(instance.id, name, value);
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
