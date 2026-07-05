#!/usr/bin/env node
// Downloads every font + its OFL.txt from google/fonts (raw.githubusercontent.com,
// which has no per-file size cap, unlike jsDelivr's 20 MB limit) and pushes each
// object into the hayate-fonts R2 bucket via `wrangler r2 object put`.
import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const BUCKET = "hayate-fonts";
const fontsDir = fileURLToPath(new URL("..", import.meta.url));

const manifest = JSON.parse(await readFile(new URL("../manifest.json", import.meta.url)));
const workdir = await mkdtemp(join(tmpdir(), "hayate-fonts-"));

for (const entry of manifest) {
  console.log(`\n== ${entry.family} ==`);
  await uploadOne(entry.sourceUrl, entry.r2Key);
  if (entry.oflUrl) {
    await uploadOne(entry.oflUrl, entry.oflR2Key);
  } else {
    console.log(`  no OFL.txt upstream (${entry.oflNote}), skipping`);
  }
}

console.log("\nDone. Deploy the worker (`pnpm run deploy`), then verify with:");
console.log("  pnpm run verify -- <worker-url>");

async function uploadOne(sourceUrl, r2Key) {
  console.log(`fetching ${sourceUrl}`);
  const res = await fetch(sourceUrl);
  if (!res.ok) {
    throw new Error(`fetch failed: ${sourceUrl} -> ${res.status}`);
  }
  const bytes = new Uint8Array(await res.arrayBuffer());
  const filePath = join(workdir, r2Key);
  await writeFile(filePath, bytes);

  console.log(`uploading ${r2Key} (${bytes.length} bytes) to r2://${BUCKET}`);
  execFileSync(
    "npx",
    ["wrangler", "r2", "object", "put", `${BUCKET}/${r2Key}`, "--file", filePath, "--remote"],
    { stdio: "inherit", cwd: fontsDir },
  );
}
