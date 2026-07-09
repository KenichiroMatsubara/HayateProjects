import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { afterEach, beforeEach, describe, expect, it } from 'vitest';

import { findConfigPath, loadTorimiConfig, normalizeConfig } from './config.js';

describe('normalizeConfig', () => {
  it('accepts a flat { build, bundle } and defaults watch to src', () => {
    expect(normalizeConfig({ build: 'vite build', bundle: 'dist/bundle.js' })).toEqual({
      build: 'vite build',
      bundle: 'dist/bundle.js',
      watch: 'src',
    });
  });

  it('honours an explicit watch dir', () => {
    expect(normalizeConfig({ build: 'b', bundle: 'o.js', watch: 'app' }).watch).toBe('app');
  });

  it('rejects a missing or empty build', () => {
    expect(() => normalizeConfig({ bundle: 'o.js' })).toThrow(/`build` must be a non-empty string/);
    expect(() => normalizeConfig({ build: '  ', bundle: 'o.js' })).toThrow(/`build`/);
  });

  it('rejects a missing bundle', () => {
    expect(() => normalizeConfig({ build: 'b' })).toThrow(/`bundle` must be a non-empty string/);
  });

  it('rejects a non-object', () => {
    expect(() => normalizeConfig(null)).toThrow(/must be an object/);
  });
});

describe('findConfigPath / loadTorimiConfig', () => {
  let dir: string;
  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'torimi-config-test-'));
  });
  afterEach(async () => {
    await rm(dir, { recursive: true, force: true });
  });

  it('throws a clear error when no config exists', async () => {
    await expect(findConfigPath(dir)).rejects.toThrow(/no torimi\.config\.mjs or torimi\.config\.js/);
  });

  it('loads the default export of torimi.config.mjs', async () => {
    await writeFile(
      join(dir, 'torimi.config.mjs'),
      "export default { build: 'vite build', bundle: 'dist-torimi/bundle.js' };\n",
    );
    expect(await loadTorimiConfig(dir)).toEqual({
      build: 'vite build',
      bundle: 'dist-torimi/bundle.js',
      watch: 'src',
    });
  });
});
