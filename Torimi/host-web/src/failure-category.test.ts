// @vitest-environment happy-dom

import type { WebHost } from '@torimi/hayate-host';
import { describe, expect, it, vi } from 'vitest';
import {
  startTorimiHost,
  TORIMI_ERROR_PANEL_ID,
  TorimiGlobalShapeError,
} from './index.js';

function fakeHost(): WebHost {
  return {
    raw: {} as WebHost['raw'],
    requestFrame: () => 0,
    cancelFrame: () => undefined,
    pipelineObservation: async () => ({
      accepted: 0,
      coalesced: 0,
      dropped: 0,
      active: false,
      pending: 0,
      failure: false,
    }),
    detach: () => undefined,
  };
}

describe('Torimi failure categories', () => {
  it('marks an uncaught runtime frame failure separately from boot failures', async () => {
    document.body.innerHTML = '';
    const onBootSettled = vi.fn();
    const handle = startTorimiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas: () => document.createElement('canvas'),
      boot: async () => fakeHost(),
      subscribe: () => ({ close() {} }),
      onBootSettled,
    });
    await vi.waitFor(() => expect(onBootSettled).toHaveBeenCalledWith({ ok: true }));

    window.dispatchEvent(
      new ErrorEvent('error', {
        error: new Error('frame exploded'),
        message: 'frame exploded',
      }),
    );

    const panel = document.getElementById(TORIMI_ERROR_PANEL_ID);
    expect(panel?.dataset.torimiFailureCategory).toBe('runtime-frame');
    expect(panel?.textContent).toContain('frame exploded');
    handle.close();
  });

  it('preserves the global-shape category when boot rejects before mount', async () => {
    document.body.innerHTML = '';
    const onBootSettled = vi.fn();
    const error = new TorimiGlobalShapeError('__torimiMount', 'function', 'object');
    const handle = startTorimiHost({
      devServerUrl: 'http://dev.example',
      hostProtocolVersion: 1,
      acquireCanvas: () => document.createElement('canvas'),
      boot: async () => Promise.reject(error),
      subscribe: () => ({ close() {} }),
      onBootSettled,
    });
    await vi.waitFor(() =>
      expect(onBootSettled).toHaveBeenCalledWith({ ok: false, error }),
    );

    const panel = document.getElementById(TORIMI_ERROR_PANEL_ID);
    expect(panel?.dataset.torimiFailureCategory).toBe('global-shape');
    handle.close();
  });
});
