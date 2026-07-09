import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { devServerContract, type LogBatch } from '@torimi/dev-server-contract';
import { createBundleDevServer, LOG_BODY_LIMIT_BYTES, type BundleDevServer } from './index.js';

/**
 * Device Log 受け口の HTTP 契約テスト。bundle 配信テストと同型に実際に listen して
 * `fetch` で叩き、ステータスコードと `onLogBatch` の呼び出しという外部挙動だけを
 * 検証する（内部の dedup 状態には触れない）（ADR-0005）。
 */
describe('POST log route', () => {
  let dir: string;
  let server: BundleDevServer;
  let origin: string;
  let onLogBatch: ReturnType<typeof vi.fn<(deviceId: string, batch: LogBatch) => void>>;

  beforeEach(async () => {
    dir = await mkdtemp(join(tmpdir(), 'torimi-dev-server-'));
    const bundlePath = join(dir, 'bundle.js');
    await writeFile(bundlePath, 'globalThis.__torimiMount = () => {};\n');
    onLogBatch = vi.fn();
    server = createBundleDevServer({ bundlePath, onLogBatch });
    origin = await server.listen();
  });

  afterEach(async () => {
    await server.close();
    await rm(dir, { recursive: true, force: true });
  });

  /** deviceId 付き log ルートへ JSON ボディを POST する。 */
  function postLog(deviceId: string, body: string): Promise<Response> {
    return fetch(`${origin}${devServerContract.logRoutePrefix}${deviceId}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body,
    });
  }

  it('accepts a valid batch with 204 and hands it to onLogBatch as (deviceId, batch)', async () => {
    const batch: LogBatch = {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 1, ts: 1720000000000, source: 'js', level: 'log', message: 'hello' }],
    };

    const res = await postLog('device-abc', JSON.stringify(batch));

    expect(res.status).toBe(204);
    expect(onLogBatch).toHaveBeenCalledWith('device-abc', batch);
  });

  it('rejects a broken-JSON body with 400 without invoking onLogBatch', async () => {
    const res = await postLog('device-abc', '{not json');

    expect(res.status).toBe(400);
    expect(onLogBatch).not.toHaveBeenCalled();
  });

  it('rejects a body whose entries is not an array with 400', async () => {
    const res = await postLog(
      'device-abc',
      JSON.stringify({ deviceLabel: 'Pixel 8', entries: 'oops' }),
    );

    expect(res.status).toBe(400);
    expect(onLogBatch).not.toHaveBeenCalled();
  });

  it('silently drops already-accepted seqs with 204 (at-least-once resend dedup)', async () => {
    const batch: LogBatch = {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 1, ts: 1720000000000, source: 'js', level: 'log', message: 'hello' }],
    };
    await postLog('device-abc', JSON.stringify(batch));

    const res = await postLog('device-abc', JSON.stringify(batch));

    expect(res.status).toBe(204);
    expect(onLogBatch).toHaveBeenCalledTimes(1);
  });

  it('delivers only the new entries when a resend mixes accepted and new seqs', async () => {
    const first: LogBatch = {
      deviceLabel: 'Pixel 8',
      entries: [
        { seq: 1, ts: 1720000000000, source: 'js', level: 'log', message: 'one' },
        { seq: 2, ts: 1720000000001, source: 'js', level: 'log', message: 'two' },
      ],
    };
    await postLog('device-abc', JSON.stringify(first));

    const resend: LogBatch = {
      deviceLabel: 'Pixel 8',
      entries: [
        { seq: 2, ts: 1720000000001, source: 'js', level: 'log', message: 'two' },
        { seq: 3, ts: 1720000000002, source: 'host', level: 'error', message: 'three' },
      ],
    };
    const res = await postLog('device-abc', JSON.stringify(resend));

    expect(res.status).toBe(204);
    expect(onLogBatch).toHaveBeenLastCalledWith('device-abc', {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 3, ts: 1720000000002, source: 'host', level: 'error', message: 'three' }],
    });
  });

  it('ignores unknown fields and still interprets known ones (additive-only, ADR-0005)', async () => {
    const res = await postLog(
      'device-abc',
      JSON.stringify({
        deviceLabel: 'Pixel 8',
        futureBatchField: true,
        entries: [
          {
            seq: 1,
            ts: 1720000000000,
            source: 'js',
            level: 'warn',
            message: 'hello',
            futureEntryField: 'ignored',
          },
        ],
      }),
    );

    expect(res.status).toBe(204);
    expect(onLogBatch).toHaveBeenCalledWith('device-abc', {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 1, ts: 1720000000000, source: 'js', level: 'warn', message: 'hello' }],
    });
  });

  it('skips entries missing known fields but delivers valid siblings (per-entry skip)', async () => {
    const res = await postLog(
      'device-abc',
      JSON.stringify({
        deviceLabel: 'Pixel 8',
        entries: [
          { seq: 1, ts: 1720000000000, source: 'js', level: 'log' }, // message 欠落
          { seq: 2, ts: 1720000000001, source: 'js', level: 'log', message: 'kept' },
        ],
      }),
    );

    expect(res.status).toBe(204);
    expect(onLogBatch).toHaveBeenCalledWith('device-abc', {
      deviceLabel: 'Pixel 8',
      entries: [{ seq: 2, ts: 1720000000001, source: 'js', level: 'log', message: 'kept' }],
    });
  });

  it('rejects a body over the named size limit with 413', async () => {
    const res = await postLog('device-abc', 'x'.repeat(LOG_BODY_LIMIT_BYTES + 1));

    expect(res.status).toBe(413);
    expect(onLogBatch).not.toHaveBeenCalled();
  });
});
