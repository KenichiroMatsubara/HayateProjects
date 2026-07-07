import { describe, expect, it } from 'vitest';
import { demoEndpointContract, devServerContract, type DemoManifest } from './index.js';

describe('devServerContract', () => {
  it('is the single wire contract dev-server and host-web both import', () => {
    expect(devServerContract).toEqual({
      bundleRoute: '/bundle.js',
      reloadRoute: '/reload',
      reloadMessage: 'reload',
    });
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
