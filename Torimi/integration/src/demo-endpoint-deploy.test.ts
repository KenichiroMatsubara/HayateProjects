import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

interface PackageJson {
  scripts?: Record<string, string>;
}

interface DemoSource {
  workspacePackage: string;
  artifactPath: string;
}

interface DemosJson {
  demos: Array<{ source: DemoSource }>;
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

  it('keeps framework demo identities neutral instead of coupling them to Todo', () => {
    const repoRoot = resolve(import.meta.dirname, '../../..');
    const demos = JSON.parse(
      readFileSync(resolve(repoRoot, 'Torimi/demo-endpoint/src/demos.json'), 'utf8'),
    ) as DemosJson;

    expect(demos.demos.map(({ source }) => source)).toEqual([
      {
        workspacePackage: '@tsubame/example-solid-demo',
        artifactPath: 'Tsubame/examples/solid-demo/dist-torimi/bundle.hermes.js',
      },
      {
        workspacePackage: '@tsubame/example-react-demo',
        artifactPath: 'Tsubame/examples/react-demo/dist-torimi/bundle.hermes.js',
      },
    ]);
    expect(existsSync(resolve(repoRoot, 'Tsubame/examples/solid-demo/package.json'))).toBe(true);
    expect(existsSync(resolve(repoRoot, 'Tsubame/examples/react-demo/package.json'))).toBe(true);
    expect(existsSync(resolve(repoRoot, 'Tsubame/examples/todo'))).toBe(false);
    expect(existsSync(resolve(repoRoot, 'Tsubame/examples/react-todo'))).toBe(false);
  });
});
