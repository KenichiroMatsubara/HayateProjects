import { describe, expect, it } from 'vitest';
import { editKeyAction, FILTERS, PRIORITIES, PRIORITY_LABEL, SORTS } from './ui/labels';
import { canReorder, FILTER_VALUES, PRIORITY_VALUES, SORT_VALUES } from './todo-model.js';

// Issue #250 wires the model's derived filter/sort/priority options into the
// Task Studio toolbar and add form. The rendering itself is verified by eye,
// but these tests guard the *seam* (#246-style): every option the model
// defines must surface as exactly one labeled control, and vice versa, so a
// new chip can never silently disappear from the UI as the model grows.

describe('toolbar filter chips', () => {
  it('exposes one labeled chip for every filter the model defines, in order', () => {
    expect(FILTERS.map((chip) => chip.value)).toEqual([...FILTER_VALUES]);
  });
});

describe('toolbar sort chips', () => {
  it('exposes one labeled chip for every sort mode the model defines, in order', () => {
    expect(SORTS.map((chip) => chip.value)).toEqual([...SORT_VALUES]);
  });
});

describe('chip labels', () => {
  it('gives every filter and sort chip a non-empty label', () => {
    for (const chip of [...FILTERS, ...SORTS]) {
      expect(chip.label.trim()).not.toBe('');
    }
  });
});

// Issue #251 adds manual reordering via up/down buttons. The buttons only make
// sense when the list is in its manual order — once name/prio sort takes over,
// the row order is derived, so moving a row up would be immediately undone. This
// seam guards that the affordance is offered for exactly the manual sort mode,
// so a future sort mode can never silently grant a meaningless reorder button.
describe('canReorder', () => {
  it('permits manual reordering only in the manual sort mode', () => {
    expect(canReorder('manual')).toBe(true);
    expect(canReorder('name')).toBe(false);
    expect(canReorder('prio')).toBe(false);
  });
});

// Inline edit keyboard contract: Enter confirms, Escape reverts. dblclick is not
// in the event vocabulary, so editing relies on keydown alone (plus blur to
// confirm, wired by hand). This seam pins the key→action mapping so a stray key
// can never accidentally commit or cancel an edit.
describe('editKeyAction', () => {
  it('commits on Enter and cancels on Escape', () => {
    expect(editKeyAction('Enter')).toBe('commit');
    expect(editKeyAction('Escape')).toBe('cancel');
  });

  it('ignores every other key', () => {
    for (const key of ['a', 'Tab', 'ArrowUp', 'Shift', ' ', '']) {
      expect(editKeyAction(key)).toBe('none');
    }
  });
});

describe('add-form priority segments', () => {
  it('offers one segment for every priority the model defines, in order', () => {
    expect(PRIORITIES).toEqual([...PRIORITY_VALUES]);
  });

  it('labels every priority segment it offers', () => {
    for (const prio of PRIORITIES) {
      expect(PRIORITY_LABEL[prio]).toBeTruthy();
    }
  });
});
