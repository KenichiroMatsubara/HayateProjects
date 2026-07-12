import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { asElementId } from '@torimi/tsubame-renderer-protocol';
import { DomRenderer } from './dom-renderer.js';
import {
  warnZOrderDivergence,
  resetZOrderDivergenceWarnings,
} from './z-order-divergence.js';

describe('warnZOrderDivergence', () => {
  beforeEach(() => {
    resetZOrderDivergenceWarnings();
    vi.stubEnv('NODE_ENV', 'development');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it('warns once per (elementId, property) in dev', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const id = asElementId(1);

    warnZOrderDivergence(id, 'opacity');
    warnZOrderDivergence(id, 'opacity');

    expect(warn).toHaveBeenCalledTimes(1);
    expect(warn.mock.calls[0]![0]).toContain('opacity');
    expect(warn.mock.calls[0]![0]).toContain('0006-dom-z-order-rn-web-emulation');
  });

  it('does not warn in production', () => {
    vi.stubEnv('NODE_ENV', 'production');
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});

    warnZOrderDivergence(asElementId(1), 'opacity');

    expect(warn).not.toHaveBeenCalled();
  });

  it('does not warn for non-registry properties', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});

    warnZOrderDivergence(asElementId(1), 'color');

    expect(warn).not.toHaveBeenCalled();
  });
});

describe('DomRenderer.setStyle – z-order divergence warnings', () => {
  beforeEach(() => {
    resetZOrderDivergenceWarnings();
    vi.stubEnv('NODE_ENV', 'development');
  });

  afterEach(() => {
    vi.unstubAllEnvs();
    vi.restoreAllMocks();
  });

  it('warns once when opacity is set via setStyle in dev', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const renderer = new DomRenderer({ document });
    const id = renderer.createElement('view');

    renderer.setStyle(id, { opacity: 0.5 });
    renderer.setStyle(id, { opacity: 0.8 });

    expect(warn).toHaveBeenCalledTimes(1);
  });

  it('does not warn in production', () => {
    vi.stubEnv('NODE_ENV', 'production');
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const renderer = new DomRenderer({ document });
    const id = renderer.createElement('view');

    renderer.setStyle(id, { opacity: 0.5 });

    expect(warn).not.toHaveBeenCalled();
  });
});
