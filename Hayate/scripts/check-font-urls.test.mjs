// check-font-urls.test.mjs — unit tests for the font-URL checker core.
// Run with: node --test Hayate/scripts/
import { test } from 'node:test';
import assert from 'node:assert/strict';

import { validateFont, checkFonts } from './check-font-urls.lib.mjs';

// A real TrueType file starts with 0x00010000; pad past MIN_FONT_BYTES so size
// and signature both look like a genuine font.
function fontBody() {
  const buf = new Uint8Array(2000);
  buf[0] = 0x00;
  buf[1] = 0x01;
  buf[2] = 0x00;
  buf[3] = 0x00;
  return buf.buffer;
}

// Minimal stand-in for the fetch Response surface validateFont touches.
function response({ status = 200, body = null } = {}) {
  return {
    ok: status >= 200 && status < 300,
    status,
    arrayBuffer: async () => body,
  };
}

test('a 200 response carrying a real font is OK', async () => {
  const fetch = async () => response({ status: 200, body: fontBody() });
  const result = await validateFont({ family: 'Noto Sans JP', url: 'x' }, { fetch });

  assert.equal(result.ok, true);
  assert.equal(result.status, 200);
  assert.equal(result.family, 'Noto Sans JP');
});

test('a 404 (moved/renamed path) is BAD', async () => {
  const fetch = async () => response({ status: 404 });
  const result = await validateFont({ family: 'Noto Sans JP', url: 'x' }, { fetch });

  assert.equal(result.ok, false);
  assert.equal(result.status, 404);
});

test('a 200 whose body is not a font (HTML error page) is BAD', async () => {
  const html = new TextEncoder().encode('<!DOCTYPE html>'.repeat(200)).buffer;
  const fetch = async () => response({ status: 200, body: html });
  const result = await validateFont({ family: 'Noto Sans JP', url: 'x' }, { fetch });

  assert.equal(result.ok, false);
  assert.equal(result.status, 200);
});

test('a 200 that is too small to be a real font is BAD', async () => {
  const buf = new Uint8Array([0x00, 0x01, 0x00, 0x00]).buffer; // valid sig, 4 bytes
  const fetch = async () => response({ status: 200, body: buf });
  const result = await validateFont({ family: 'Noto Sans JP', url: 'x' }, { fetch });

  assert.equal(result.ok, false);
});

test('a transient network error is retried, then succeeds', async () => {
  let calls = 0;
  const fetch = async () => {
    calls++;
    if (calls === 1) throw new Error('ECONNRESET');
    return response({ status: 200, body: fontBody() });
  };
  const result = await validateFont(
    { family: 'Noto Sans JP', url: 'x' },
    { fetch, retries: 3, sleep: async () => {} },
  );

  assert.equal(result.ok, true);
  assert.equal(result.attempts, 2);
});

test('a 404 is not retried — it fails fast on the first attempt', async () => {
  let calls = 0;
  const fetch = async () => {
    calls++;
    return response({ status: 404 });
  };
  const result = await validateFont(
    { family: 'Noto Sans JP', url: 'x' },
    { fetch, retries: 3, sleep: async () => {} },
  );

  assert.equal(result.ok, false);
  assert.equal(result.attempts, 1);
  assert.equal(calls, 1);
});

test('persistent transient failures exhaust retries, then report BAD', async () => {
  let calls = 0;
  const fetch = async () => {
    calls++;
    return response({ status: 503 });
  };
  const result = await validateFont(
    { family: 'Noto Sans JP', url: 'x' },
    { fetch, retries: 2, sleep: async () => {} },
  );

  assert.equal(result.ok, false);
  assert.equal(result.status, 503);
  assert.equal(result.attempts, 3); // 1 initial + 2 retries
  assert.equal(calls, 3);
});

test('a request that exceeds the timeout is aborted and reported BAD', async () => {
  // A fetch that never resolves on its own; it only settles when aborted.
  const fetch = (_url, { signal }) =>
    new Promise((_resolve, reject) => {
      signal.addEventListener('abort', () =>
        reject(signal.reason ?? new Error('aborted')),
      );
    });
  const result = await validateFont(
    { family: 'Noto Sans JP', url: 'x' },
    { fetch, retries: 0, timeoutMs: 5, sleep: async () => {} },
  );

  assert.equal(result.ok, false);
  assert.ok(result.error, 'an aborted request records an error');
});

test('checkFonts reports the failing count across a manifest', async () => {
  const fonts = [
    { family: 'Good', url: 'good' },
    { family: 'Dead', url: 'dead' },
  ];
  const fetch = async (url) =>
    url === 'good'
      ? response({ status: 200, body: fontBody() })
      : response({ status: 404 });

  const { bad, total } = await checkFonts(fonts, { fetch, sleep: async () => {} });

  assert.equal(total, 2);
  assert.equal(bad, 1);
});
