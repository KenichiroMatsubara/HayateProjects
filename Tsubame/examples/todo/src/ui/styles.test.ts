import { describe, expect, it } from 'vitest';
import { palette } from '../theme';
import { TITLE_MAX_LINES, TITLE_OVERFLOW, TITLE_TEXT_OVERFLOW, titleStyle } from './styles';

// Issue #428: the todo row title sits in a flexGrow:1 column and competes with
// the priority label and the arrow/delete controls for horizontal space. Without
// an explicit overflow policy it wraps and pushes the row taller. These seams pin
// the policy — a bounded line count, ellipsised, with hidden overflow — so the
// title can never silently start wrapping freely again. Values are placeholders;
// the final clamp policy is a manual follow-up (named constants, no magic numbers).

const p = palette('light', 'teal');

describe('todo title overflow policy', () => {
  it('clamps the title to a finite, positive number of lines', () => {
    expect(Number.isInteger(TITLE_MAX_LINES)).toBe(true);
    expect(TITLE_MAX_LINES).toBeGreaterThan(0);
    expect(titleStyle(p, false).maxLines).toBe(TITLE_MAX_LINES);
  });

  it('trails an ellipsis on an overflowing title instead of revealing a hard cut', () => {
    expect(TITLE_TEXT_OVERFLOW).toBe('ellipsis');
    expect(titleStyle(p, false).textOverflow).toBe(TITLE_TEXT_OVERFLOW);
  });

  it('hides overflow so a long title is clamped rather than spilling and growing the row', () => {
    expect(TITLE_OVERFLOW).toBe('hidden');
    expect(titleStyle(p, false).overflow).toBe(TITLE_OVERFLOW);
  });

  it('applies the same clamp whether the todo is done or active', () => {
    const active = titleStyle(p, false);
    const done = titleStyle(p, true);
    for (const key of ['maxLines', 'textOverflow', 'overflow'] as const) {
      expect(done[key]).toBe(active[key]);
    }
  });
});
