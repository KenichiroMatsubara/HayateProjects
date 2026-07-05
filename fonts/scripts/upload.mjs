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

const missingOfl = [];

for (const entry of manifest) {
  console.log(`\n== ${entry.family} ==`);
  await uploadOne(entry.sourceUrl, entry.r2Key);
  try {
    await uploadOne(entry.oflUrl, entry.oflR2Key);
  } catch (err) {
    // Not every google/fonts directory has an OFL.txt (e.g. mplusrounded1c) even
    // though METADATA.pb declares an OFL license — an upstream gap, not ours.
    // Missing license text doesn't block serving the font, so don't abort the run.
    console.warn(`  skipping OFL.txt for ${entry.family}: ${err.message}`);
    missingOfl.push(entry.family);
  }
}

console.log("\nDone. Deploy the worker (`pnpm run deploy`), then verify with:");
console.log("  pnpm run verify -- <worker-url>");
if (missingOfl.length > 0) {
  console.log(`\nNo OFL.txt found upstream for: ${missingOfl.join(", ")}`);
  console.log("(METADATA.pb still declares an OFL license for these — check google/fonts directly if this matters.)");
}

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
