import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { describe, expect, it } from 'vitest';

import { countClassKeywords, lowerFileTo, lowerForHermes } from './lower.js';

describe('lowerForHermes', () => {
  it('lowers a class expression to ES5-equivalent so Hermes can evaluate it (ADR-0112)', async () => {
    const lowered = await lowerForHermes('const X = class { greet() { return 1; } };\nexport { X };\n');
    // The anonymous class expression (`= class {…}`) — which Hermes mis-evaluates
    // to undefined on device — is gone after lowering. (preset-env may still emit a
    // stray `class` token elsewhere, exactly as the real bundle build reports, so we
    // assert the specific problematic form is gone rather than a zero count.)
    expect(lowered).not.toMatch(/=\s*class\b/);
  });
});

describe('countClassKeywords', () => {
  it('counts class keywords, including a leading one, but ignores property access / identifiers', () => {
    expect(countClassKeywords('class A{} class B{}')).toBe(2);
    expect(countClassKeywords('el.className = 1')).toBe(0);
    expect(countClassKeywords('foo.class')).toBe(0);
  });
});

describe('lowerFileTo', () => {
  it('writes the lowered result to a separate destination path', async () => {
    const dir = await mkdtemp(join(tmpdir(), 'torimi-lower-test-'));
    try {
      const src = join(dir, 'bundle.js');
      const dest = join(dir, 'bundle.hermes.js');
      await writeFile(src, 'const A = class {};\n');
      const result = await lowerFileTo(src, dest);
      expect(result.size).toBeGreaterThan(0);
      // src stays un-lowered; only dest carries the lowered output.
      expect(await readFile(src, 'utf8')).toMatch(/class/);
      expect(await readFile(dest, 'utf8')).not.toMatch(/=\s*class\b/);
    } finally {
      await rm(dir, { recursive: true, force: true });
    }
  });
});
