import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

interface PackageJson {
  scripts?: Record<string, string>;
}

describe('Demo Endpoint manual deploy', () => {
  it('rebuilds every manifest demo before wrangler can publish public/', () => {
    const packageJson = JSON.parse(
      readFileSync(resolve(import.meta.dirname, '../../demo-endpoint/package.json'), 'utf8'),
    ) as PackageJson;
    const deploy = packageJson.scripts?.deploy ?? '';

    const buildIndex = deploy.indexOf('build:demos');
    const wranglerIndex = deploy.indexOf('wrangler deploy');
    expect(buildIndex, 'deploy must rebuild demos instead of trusting stale public/').toBeGreaterThanOrEqual(0);
    expect(wranglerIndex, 'deploy must publish through wrangler').toBeGreaterThanOrEqual(0);
    expect(buildIndex).toBeLessThan(wranglerIndex);
  });
});
