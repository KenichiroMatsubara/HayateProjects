#!/usr/bin/env node
// Fetches every uploaded object through the deployed worker URL and reports
// which ones fail, so this can be run before repointing Hayate's fonts.json.
import { readFile } from "node:fs/promises";

// pnpm forwards a literal "--" along with args after `pnpm run verify -- <url>`
// instead of stripping it, so filter it out rather than trusting argv[2].
const baseUrl = process.argv.slice(2).filter((a) => a !== "--")[0];
if (!baseUrl) {
  console.error("usage: node scripts/verify.mjs <worker-base-url>");
  process.exit(1);
}

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url)));

let failed = 0;
for (const entry of manifest) {
  for (const key of [entry.r2Key, entry.oflR2Key].filter(Boolean)) {
    const url = `${baseUrl.replace(/\/$/, "")}/${key}`;
    try {
      const res = await fetch(url);
      if (!res.ok) failed++;
      console.log(`${res.ok ? "ok  " : "FAIL"} ${res.status} ${key}`);
    } catch (err) {
      failed++;
      console.log(`FAIL --- ${key} (${err.message})`);
    }
  }
}

if (failed > 0) {
  console.error(`\n${failed} object(s) failed to fetch`);
  process.exit(1);
}
console.log("\nall objects fetched successfully");
