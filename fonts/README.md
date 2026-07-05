# hayate-fonts

Self-hosted font CDN (Cloudflare R2 + Worker) replacing the jsDelivr `google/fonts`
mirror Hayate's web adapter used to fetch fonts from. See [ADR-0139](../Hayate/docs/adr/0139-self-hosted-font-cdn-cloudflare-r2-worker.md).

Fonts are a **pinned snapshot** — there is no automatic sync with upstream Google
Fonts. `manifest.json` records where each file was sourced from for provenance
and future re-uploads.

## One-time setup

```
wrangler login
npx wrangler r2 bucket create hayate-fonts
pnpm run upload
pnpm run deploy
pnpm run verify -- https://<worker-name>.<your-subdomain>.workers.dev
pnpm run apply-fonts-json -- https://<worker-name>.<your-subdomain>.workers.dev
```

`apply-fonts-json` overwrites `Hayate/crates/platform/web/fonts.json`; review
the diff and commit it once `verify` is clean.

## Updating a font

Edit its `sourceUrl` in `manifest.json` if needed, then re-run `pnpm run upload`
for that entry (or all of them — uploads are idempotent overwrites).

## Adding a new font

Add an entry to `manifest.json` (see existing entries for the shape), run
`pnpm run upload`, then `pnpm run apply-fonts-json`.
