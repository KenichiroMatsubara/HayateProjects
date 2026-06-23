#!/usr/bin/env node
// scripts/check-font-urls.mjs — validate every URL in the web adapter's
// fonts.json actually resolves to a real font file.
//
// google/fonts moves files around (license dir ofl/↔apache/, variable-font
// axis tags like [slnt,wght]→[opsz,wght]); a stale URL 404s and the on-demand
// fallback silently dead-ends. Run this whenever fonts.json changes:
//
//   node Hayate/scripts/check-font-urls.mjs
//
// Exits non-zero if any URL is not a 200 with a TrueType/OpenType signature.
// The validation logic lives in check-font-urls.lib.mjs so it can be unit-tested
// with a fake fetch (transient failures are retried; 404s fail fast).

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

import { checkFonts } from './check-font-urls.lib.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const manifest = join(here, '../crates/platform/web/fonts.json');
const fonts = JSON.parse(readFileSync(manifest, 'utf8'));

const { bad, total } = await checkFonts(fonts, {
  onResult: (r) => {
    const size = (r.ok ? `${r.bytes}B` : '-').padStart(10);
    const status = String(r.status || 'ERR').padEnd(3);
    const note = r.attempts > 1 ? ` (${r.attempts} attempts)` : '';
    const tail = r.error ? ` -> ${r.error}` : '';
    console.log(`${r.ok ? 'OK ' : 'BAD'} ${status} ${size} sig=${r.sig} :: ${r.family}${note}${tail}`);
  },
});

console.log(`\n${bad} bad / ${total} total`);
process.exit(bad === 0 ? 0 : 1);
