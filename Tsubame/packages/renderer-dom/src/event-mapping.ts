import type { EventKind } from '@tsubame/renderer-protocol';

/**
 * Tsubame の {@link EventKind} からネイティブ DOM イベント名へのマッピング。
 *
 * hover-enter / hover-leave はバブリングしない mouseenter / mouseleave に
 * 対応させ、要素単位の意味論（Interaction Event）を保つ。
 */
export const DOM_EVENT_NAME: Record<EventKind, string> = {
  click: 'click',
  input: 'input',
  change: 'change',
  keydown: 'keydown',
  'hover-enter': 'mouseenter',
  'hover-leave': 'mouseleave',
  focus: 'focus',
  blur: 'blur',
};
