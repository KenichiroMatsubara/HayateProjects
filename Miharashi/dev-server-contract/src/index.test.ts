import { describe, expect, it } from 'vitest';
import { devServerContract } from './index.js';

describe('devServerContract', () => {
  it('is the single wire contract dev-server and host-web both import', () => {
    expect(devServerContract).toEqual({
      bundleRoute: '/bundle.js',
      reloadRoute: '/reload',
      reloadMessage: 'reload',
    });
  });
});
