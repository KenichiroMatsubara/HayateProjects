import { describe, expect, it } from 'vitest';
import {
  demoEndpointContract,
  devServerContract,
  type DemoManifest,
  type LogBatch,
} from './index.js';

describe('devServerContract', () => {
  it('is the single wire contract dev-server and host-web both import', () => {
    expect(devServerContract).toEqual({
      bundleRoute: '/bundle.js',
      reloadRoute: '/reload',
      reloadMessage: 'reload',
      logRoutePrefix: '/log/',
    });
  });

  it('types a Device Log batch as entries the dev-server receiver consumes (ADR-0005)', () => {
    const batch: LogBatch = {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 1, ts: 1720000000000, source: 'js', level: 'error', message: 'boom' }],
    };
    expect(batch.entries[0]?.level).toBe('error');
  });
});

describe('demoEndpointContract', () => {
  it('is the wire contract demo-endpoint and hosts both import (ADR-0003)', () => {
    expect(demoEndpointContract).toEqual({
      demoManifestRoute: '/demos.json',
    });
  });

  it('types the Demo Manifest as named entries the host menu and first-demo autoload consume', () => {
    const manifest: DemoManifest = {
      demos: [{ name: 'Todo (Solid)', bundleUrl: '/solid/bundle.js' }],
    };
    expect(manifest.demos[0]?.bundleUrl).toBe('/solid/bundle.js');
  });
});
