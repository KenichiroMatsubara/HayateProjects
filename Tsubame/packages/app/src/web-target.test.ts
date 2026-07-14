import { describe, expect, it } from 'vitest';
import { shouldUseDomRenderer } from './web-target.js';

describe('shouldUseDomRenderer', () => {
  it('uses DOM for the explicit DOM escape hatch', () => {
    expect(shouldUseDomRenderer('?renderer=dom', { hasEditContext: true })).toBe(true);
  });

  it('uses DOM when the browser cannot support editable Hayate content', () => {
    expect(shouldUseDomRenderer('', { hasEditContext: false })).toBe(true);
  });

  it('otherwise leaves all renderer selection to the Hayate host', () => {
    expect(shouldUseDomRenderer('', { hasEditContext: true })).toBe(false);
    expect(shouldUseDomRenderer('?renderer=host-owned-value', { hasEditContext: true })).toBe(false);
  });
});
