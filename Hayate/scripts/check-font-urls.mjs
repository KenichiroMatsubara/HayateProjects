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

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const manifest = join(here, '../crates/adapters/web/fonts.json');
const fonts = JSON.parse(readFileSync(manifest, 'utf8'));

const SIGS = new Set(['00010000', '4f54544f' /* OTTO */, '74727565' /* true */, '74746366' /* ttcf */]);

let bad = 0;
for (const f of fonts) {
  try {
    const r = await fetch(f.url, { headers: { 'User-Agent': 'Mozilla/5.0' } });
    const ab = r.ok ? await r.arrayBuffer() : null;
    const sig = ab
      ? [...new Uint8Array(ab).slice(0, 4)].map((b) => b.toString(16).padStart(2, '0')).join('')
      : '-';
    const ok = r.ok && ab && ab.byteLength > 1000 && SIGS.has(sig);
    if (!ok) bad++;
    console.log(`${ok ? 'OK ' : 'BAD'} ${String(r.status).padEnd(3)} ${(ab ? ab.byteLength + 'B' : '-').padStart(10)} sig=${sig} :: ${f.family}`);
  } catch (e) {
    bad++;
    console.log(`BAD ERR :: ${f.family} -> ${e.message}`);
  }
}
console.log(`\n${bad} bad / ${fonts.length} total`);
process.exit(bad === 0 ? 0 : 1);
