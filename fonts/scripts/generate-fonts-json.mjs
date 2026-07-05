#!/usr/bin/env node
// Regenerates Hayate/crates/platform/web/fonts.json from manifest.json, pointed
// at the given worker base URL. Run after upload + verify succeed.
import { readFile, writeFile } from "node:fs/promises";

// pnpm forwards a literal "--" along with args after `pnpm run ... -- <url>`
// instead of stripping it, so filter it out rather than trusting argv[2].
const baseUrl = process.argv.slice(2).filter((a) => a !== "--")[0];
if (!baseUrl) {
  console.error("usage: node scripts/generate-fonts-json.mjs <worker-base-url>");
  process.exit(1);
}
const trimmedBase = baseUrl.replace(/\/$/, "");

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url)));

const fontsJson = manifest.map(({ family, r2Key, scripts }) => {
  const entry = { family, url: `${trimmedBase}/${r2Key}` };
  if (scripts.length > 0) entry.scripts = scripts;
  return entry;
});

const target = new URL("../../Hayate/crates/platform/web/fonts.json", import.meta.url);
await writeFile(target, `${JSON.stringify(fontsJson, null, 2)}\n`);
console.log(`wrote ${fontsJson.length} entries to ${target.pathname}`);
