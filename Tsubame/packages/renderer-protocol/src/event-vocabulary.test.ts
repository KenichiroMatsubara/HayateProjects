import { describe, it, expect } from 'vitest';
import { EVENT_PROP, REJECTED_EVENT_PROPS } from './index.js';

// Tsubame Adapter（solid / react / vue）が共有するイベント語彙の正本は
// renderer-protocol に置く（ADR-0010）。ここでは公開 export と対応表が
// 現行仕様どおりであることだけを検証する。

describe('adapter event vocabulary (Tsubame ADR-0010, #483)', () => {
  it('maps authoring prop names to EventKind (on + PascalCase 規約)', () => {
    expect(EVENT_PROP).toEqual({
      onClick: 'click',
      onInput: 'input',
      onKeyDown: 'keydown',
      onFocus: 'focus',
      onBlur: 'blur',
      onPointerDown: 'pointerdown',
      onPointerMove: 'pointermove',
      onPointerUp: 'pointerup',
    });
  });

  it('rejects the hover event props (ADR-0059)', () => {
    expect(REJECTED_EVENT_PROPS.has('onHoverEnter')).toBe(true);
    expect(REJECTED_EVENT_PROPS.has('onHoverLeave')).toBe(true);
    expect(REJECTED_EVENT_PROPS.size).toBe(2);
  });
});
