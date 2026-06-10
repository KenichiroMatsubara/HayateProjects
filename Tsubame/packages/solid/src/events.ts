import type { EventKind } from '@tsubame/renderer-protocol';

/**
 * Tsubame Adapter が拒否する JSX イベント prop（ADR-0059）。
 * 視覚ホバーは `style` 内の `:hover` を使う。
 */
export const REJECTED_EVENT_PROPS: ReadonlySet<string> = new Set([
  'onHoverEnter',
  'onHoverLeave',
]);

/**
 * JSX の prop 名から Tsubame の {@link EventKind} へのマッピング。
 * `on` + PascalCase 規約に従う。
 */
export const EVENT_PROP: Record<string, EventKind> = {
  onClick: 'click',
  onInput: 'input',
  onKeyDown: 'keydown',
  onFocus: 'focus',
  onBlur: 'blur',
};
