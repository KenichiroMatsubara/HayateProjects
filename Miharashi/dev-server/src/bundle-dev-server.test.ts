import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { BUNDLE_ROUTE, createBundleDevServer, type BundleDevServer } from './index.js';

/**
 * 最小 dev server の契約テスト。実際に listen して `fetch` で叩き、「単一 App Bundle を
 * HTTP で配信するだけ」を本物の HTTP 経路で確認する（CONTEXT.md / ADR-0001 のスライス #1）。
 */
describe('createBundleDevServer', () => {
  let dir: string;
  let bundlePath: string;
  let server: BundleDevServer;
  let origin: string;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'miharashi-dev-server-'));
    bundlePath = join(dir, 'bundle.js');
    await writeFile(bundlePath, 'globalThis.__miharashiMount = () => {};\n');
    server = createBundleDevServer({ bundlePath });
    origin = await server.listen();
  });

  afterEach(async () => {
    await server.close();
    await rm(dir, { recursive: true, force: true });
  });

  it('serves the App Bundle at the bundle route over HTTP', async () => {
    const res = await fetch(`${origin}${BUNDLE_ROUTE}`);

    expect(res.status).toBe(200);
    expect(await res.text()).toBe('globalThis.__miharashiMount = () => {};\n');
  });

  it('serves the bundle with a JavaScript content-type', async () => {
    const res = await fetch(`${origin}${BUNDLE_ROUTE}`);

    expect(res.headers.get('content-type')).toMatch(/javascript/);
  });

  it('allows a cross-origin host page to fetch the bundle (CORS)', async () => {
    // ホストページは別 origin（examples/todo の vite）で動き、fetch でバンドルを取りに来る。
    const res = await fetch(`${origin}${BUNDLE_ROUTE}`);

    expect(res.headers.get('access-control-allow-origin')).toBe('*');
  });

  it('returns 404 for unknown paths', async () => {
    const res = await fetch(`${origin}/nope.js`);

    expect(res.status).toBe(404);
  });
});
