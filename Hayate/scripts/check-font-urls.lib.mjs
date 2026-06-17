// check-font-urls.lib.mjs — pure, testable core for the font-URL checker.
//
// Kept free of filesystem and process concerns so it can be unit-tested with a
// fake fetch (see check-font-urls.test.mjs). The CLI wrapper in
// check-font-urls.mjs wires in the real manifest, `globalThis.fetch`, and the
// process exit code.

// Leading 4 bytes that mark a TrueType/OpenType container. A 200 response whose
// body does not start with one of these is google/fonts serving an HTML error
// page (or an LFS pointer), not a font — treat it as a failure.
export const FONT_SIGNATURES = new Set([
  '00010000', // TrueType outlines
  '4f54544f', // 'OTTO' — CFF/OpenType outlines
  '74727565', // 'true'
  '74746366', // 'ttcf' — TrueType collection
]);

// Real Noto/Inter/etc. files are tens of KB and up; anything tiny is an error
// page that happened to start with plausible bytes.
export const MIN_FONT_BYTES = 1000;

export function fontSignature(arrayBuffer) {
  return [...new Uint8Array(arrayBuffer).slice(0, 4)]
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

// A response status worth retrying: jsdelivr throttling (429) and transient
// upstream/CDN errors (5xx). A 404 is NOT transient — the path really moved, and
// retrying it would only mask the very breakage this checker exists to catch.
function isTransientStatus(status) {
  return status === 429 || status >= 500;
}

// One fetch + body inspection, no retry. Aborts after `timeoutMs` so a hung
// connection can't stall CI. Returns a result describing the attempt;
// `transient` flags whether a retry could plausibly help.
async function attemptFetch(font, fetch, timeoutMs) {
  const controller = new AbortController();
  const timer = setTimeout(
    () => controller.abort(new Error(`timeout after ${timeoutMs}ms`)),
    timeoutMs,
  );
  let r;
  try {
    r = await fetch(font.url, {
      headers: { 'User-Agent': 'Mozilla/5.0' },
      signal: controller.signal,
    });
  } finally {
    clearTimeout(timer);
  }
  if (!r.ok) {
    return {
      ok: false,
      status: r.status,
      bytes: 0,
      sig: '-',
      transient: isTransientStatus(r.status),
    };
  }
  const ab = await r.arrayBuffer();
  const sig = fontSignature(ab);
  const bytes = ab.byteLength;
  const ok = bytes >= MIN_FONT_BYTES && FONT_SIGNATURES.has(sig);
  // A 200 with the wrong body is a content problem, not a network blip — don't
  // retry it.
  return { ok, status: r.status, bytes, sig, transient: false };
}

// Validate a single font entry, retrying transient failures with backoff.
// Returns a plain result object; never throws.
//
//   { family, ok, status, bytes, sig, attempts, error }
export async function validateFont(font, opts = {}) {
  const {
    fetch = globalThis.fetch,
    retries = 3,
    timeoutMs = 20000,
    sleep = (ms) => new Promise((r) => setTimeout(r, ms)),
    backoffMs = (attempt) => 500 * 2 ** (attempt - 1),
  } = opts;

  let attempts = 0;
  let last = { ok: false, status: 0, bytes: 0, sig: '-', error: undefined };

  for (let attempt = 1; attempt <= retries + 1; attempt++) {
    attempts = attempt;
    try {
      const r = await attemptFetch(font, fetch, timeoutMs);
      last = { ...r };
      if (r.ok || !r.transient) break;
    } catch (e) {
      // Thrown errors (DNS, reset, abort/timeout) are transient.
      last = { ok: false, status: 0, bytes: 0, sig: '-', error: e.message };
    }
    if (attempt <= retries) await sleep(backoffMs(attempt));
  }

  return { family: font.family, attempts, ...last };
}

// Validate every font in a manifest. Runs them concurrently and returns the
// per-font results plus a count of failures, so the CLI can log each line and
// exit non-zero when `bad > 0`. The optional `onResult` hook lets the CLI stream
// progress as each check settles.
export async function checkFonts(fonts, opts = {}) {
  const { onResult, ...fontOpts } = opts;
  const results = await Promise.all(
    fonts.map(async (font) => {
      const result = await validateFont(font, fontOpts);
      if (onResult) onResult(result);
      return result;
    }),
  );
  const bad = results.filter((r) => !r.ok).length;
  return { results, bad, total: fonts.length };
}
