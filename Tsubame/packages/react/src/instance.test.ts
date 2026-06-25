import { describe, it, expect } from 'vitest';
import { asElementId } from '@tsubame/renderer-protocol';
import { createInstance } from './instance.js';

describe('@tsubame/react host instance', () => {
  it('is structure-zero: holds id/kind/listeners but no parent/children (ADR-0062)', () => {
    const inst = createInstance(asElementId(1), 'view');

    // 構造（親子）は Fiber tree が持つ。instance には shadow tree を持たせない。
    expect('parent' in inst).toBe(false);
    expect('children' in inst).toBe(false);

    expect(inst.id).toBe(1);
    expect(inst.kind).toBe('view');
    expect(inst.listeners).toBeInstanceOf(Map);
    expect(inst.listeners.size).toBe(0);

    // id / kind / listeners 以外のフィールドは持たない
    expect(Object.keys(inst).sort()).toEqual(['id', 'kind', 'listeners']);
  });
});
