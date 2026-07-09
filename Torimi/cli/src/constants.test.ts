import { describe, expect, it } from 'vitest';

import {
  DEFAULT_TARGET,
  NATIVE_DEV_PORT,
  WEB_DEV_PORT,
  loweredBundlePath,
  portForTarget,
  resolveTarget,
} from './constants.js';

describe('resolveTarget', () => {
  it('defaults to native when no target is given (ADR-0008 §2)', () => {
    expect(resolveTarget(undefined)).toBe('native');
    expect(DEFAULT_TARGET).toBe('native');
  });

  it('accepts native and web', () => {
    expect(resolveTarget('native')).toBe('native');
    expect(resolveTarget('web')).toBe('web');
  });

  it('rejects an unknown target', () => {
    expect(() => resolveTarget('ios')).toThrow(/unknown target "ios"/);
  });
});

describe('portForTarget', () => {
  it('maps native/web to their named default ports (no magic numbers)', () => {
    expect(portForTarget('native')).toBe(NATIVE_DEV_PORT);
    expect(portForTarget('web')).toBe(WEB_DEV_PORT);
    expect(NATIVE_DEV_PORT).toBe(5179);
    expect(WEB_DEV_PORT).toBe(5181);
  });
});

describe('loweredBundlePath', () => {
  it('inserts .hermes before the extension so the lowered bundle is a separate path', () => {
    expect(loweredBundlePath('dist-torimi/bundle.js')).toBe('dist-torimi/bundle.hermes.js');
  });

  it('appends .hermes when there is no extension', () => {
    expect(loweredBundlePath('dist/bundle')).toBe('dist/bundle.hermes');
  });
});
