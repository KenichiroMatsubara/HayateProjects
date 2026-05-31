import type { EventKind } from '@tsubame/renderer-protocol';

/**
 * JSX の prop 名から Tsubame の {@link EventKind} へのマッピング。
 * `on` + PascalCase 規約に従う。
 */
export const EVENT_PROP: Record<string, EventKind> = {
  onClick: 'click',
  onHoverEnter: 'hover-enter',
  onHoverLeave: 'hover-leave',
  onFocus: 'focus',
  onBlur: 'blur',
};
