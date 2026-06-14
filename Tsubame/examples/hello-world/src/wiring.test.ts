import { describe, expect, it } from 'vitest';
import { FILTERS, PRIORITIES, PRIORITY_LABEL, SORTS } from './App';
import { FILTER_VALUES, PRIORITY_VALUES, SORT_VALUES } from './todo-model.js';

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
