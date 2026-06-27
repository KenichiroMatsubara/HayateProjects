import type { ElementId, ElementKind } from './element.js';
import type { IRenderer } from './renderer.js';
import type { ViewportCondition } from './viewport-condition.js';
import type { EventHandler, Unsubscribe } from './event.js';
import { EVENT_PROP, REJECTED_EVENT_PROPS } from './event.js';
import type { PseudoStyleKey } from './pseudo-style.js';
import { splitHayateStyle } from './pseudo-style.js';
import { assertKnownElementProperty } from './property.js';

/**
 * `applyElementProp` が触る要素ハンドルの最小サーフェス。`tsubame-solid` の
 * `TsubameNode`（shadow tree 用に parent/children も持つ）と `tsubame-react` の
 * `TsubameInstance`（構造ゼロ）はどちらも構造的にこれを満たす。FW 固有の tree 構造は
 * この seam に現れない（ADR-0010 / ADR-0062）。
 */
export interface PropTarget {
  readonly id: ElementId;
  readonly kind: ElementKind;
  /** event prop 名 → 解除関数。同名 event prop の差し替え時に旧購読を解除する。 */
  readonly listeners: Map<string, Unsubscribe>;
}

/** FW 予約／構造に関与する prop（ホストへは流さない）。solid は `key` を受けない。 */
const STRUCTURAL_PROPS: ReadonlySet<string> = new Set(['children', 'ref', 'key']);

/**
 * 1 つの prop の変化を `IRenderer` へ適用する Tsubame Adapter 共通の seam。
 * style チャンネル分割・viewport variant・閉じた event 語彙・閉じた要素プロパティの
 * dispatch ladder を単独所有する。solid / react / 将来 vue はこの 1 関数を呼ぶだけで、
 * 差分計算・リスナの一括解体・tree 配線など FW 固有の orchestration は各 adapter に残る。
 *
 * 呼び出し側は per-prop で冪等に呼ぶ前提（差分は FW reconciler 側で済ませる）。
 */
export function applyElementProp(
  renderer: IRenderer,
  target: PropTarget,
  name: string,
  value: unknown,
): void {
  if (STRUCTURAL_PROPS.has(name)) return;

  // text も Hayate element なので style は適用する（ADR-0058）。
  if (name === 'style') {
    const { base, pseudo } = splitHayateStyle((value ?? {}) as Record<string, unknown>);
    renderer.setStyle(target.id, base);
    for (const [key, block] of Object.entries(pseudo)) {
      if (block !== undefined) {
        renderer.setPseudoStyle(target.id, key as PseudoStyleKey, block);
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
      renderer.setStyleVariant(target.id, variant.condition, base);
    }
    return;
  }

  // text 要素は style 以外のプロパティを持たない。
  if (target.kind === 'text') return;

  if (REJECTED_EVENT_PROPS.has(name)) {
    throw new Error(
      `${name} is not supported as an event prop. Use ':hover' / ':active' / ':focus' in style for visual feedback (ADR-0056, ADR-0059).`,
    );
  }

  const eventKind = EVENT_PROP[name];
  if (eventKind !== undefined) {
    target.listeners.get(name)?.();
    target.listeners.delete(name);
    if (typeof value === 'function') {
      target.listeners.set(
        name,
        renderer.addEventListener(target.id, eventKind, value as EventHandler),
      );
    }
    return;
  }

  assertKnownElementProperty(name);
  renderer.setProperty(target.id, name, value);
}
