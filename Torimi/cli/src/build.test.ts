import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { buildForTarget, runShell } from './build.js';

// Drives the whole build→(lower) pipeline with a fake opaque `build` command (a
// shell one-liner that writes the bundle), proving the CLI stays build-tool blind
// and that native lowers to a SEPARATE path while web serves the build output.
describe('buildForTarget', () => {
  let dir: string;
  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'torimi-build-test-'));
  });
  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  const config = (extra = {}) => ({
    build: "mkdir -p dist-torimi && printf 'const A = class {};\\n' > dist-torimi/bundle.js",
    bundle: 'dist-torimi/bundle.js',
    watch: 'src',
    ...extra,
  });

  it('web: runs the opaque build and returns the un-lowered bundle path', async () => {
    const out = await buildForTarget(config(), 'web', dir);
    expect(out).toBe(join(dir, 'dist-torimi/bundle.js'));
    expect(await readFile(out, 'utf8')).toMatch(/class/); // un-lowered
  });

  it('native: builds, then lowers to a separate .hermes path (never serves un-lowered)', async () => {
    const out = await buildForTarget(config(), 'native', dir);
    expect(out).toBe(join(dir, 'dist-torimi/bundle.hermes.js'));
    // the lowered artifact has no class expression…
    expect(await readFile(out, 'utf8')).not.toMatch(/=\s*class\b/);
    // …and the original build output is left untouched (un-lowered) at its own path.
    expect(await readFile(join(dir, 'dist-torimi/bundle.js'), 'utf8')).toMatch(/class/);
  });

  it('propagates a failing build command as an error', async () => {
    await writeFile(join(dir, 'noop'), '');
    await expect(runShell('exit 3', dir)).rejects.toThrow(/exit 3/);
  });
});
