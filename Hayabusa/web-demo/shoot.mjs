// Hayabusa→Web present デモを headless Chromium で開き、canvas をスクショする。
// 使い方: node shoot.mjs  （web-demo ディレクトリで実行）
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { extname, join, normalize } from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire('/opt/node22/lib/node_modules/');
const { chromium } = require('playwright');

const ROOT = process.cwd();
const PORT = 5199;

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.wasm': 'application/wasm',
  '.ttf': 'font/ttf',
  '.json': 'application/json',
};

const server = createServer(async (req, res) => {
  try {
    const urlPath = decodeURIComponent(req.url.split('?')[0]);
    const rel = urlPath === '/' ? '/index.html' : urlPath;
    const file = normalize(join(ROOT, rel));
    if (!file.startsWith(ROOT)) {
      res.writeHead(403).end('forbidden');
      return;
    }
    const body = await readFile(file);
    res.writeHead(200, { 'content-type': MIME[extname(file)] ?? 'application/octet-stream' });
    res.end(body);
  } catch (e) {
    res.writeHead(404).end(String(e));
  }
});

await new Promise((r) => server.listen(PORT, r));
console.log(`serving ${ROOT} on http://localhost:${PORT}`);

const browser = await chromium.launch({
  executablePath: join(process.env.PLAYWRIGHT_BROWSERS_PATH ?? '/opt/pw-browsers', 'chromium-1194', 'chrome-linux', 'chrome'),
  args: ['--no-sandbox', '--use-gl=swiftshader', '--enable-unsafe-swiftshader'],
});
const page = await browser.newPage({ deviceScaleFactor: 2 });
page.on('console', (m) => console.log('  [page]', m.type(), m.text()));
page.on('pageerror', (e) => console.log('  [pageerror]', e.message));

await page.goto(`http://localhost:${PORT}/`, { waitUntil: 'load' });
try {
  await page.waitForSelector('html[data-hayabusa-ready="1"]', { timeout: 15000 });
  console.log('hayabusa ready');
} catch {
  console.log('WARN: ready flag not seen within timeout — shooting anyway');
}
await page.waitForTimeout(500);

const out = join(ROOT, 'hayabusa-web.png');
await page.locator('#app').screenshot({ path: out });
console.log('screenshot →', out);

// ページ全体も保存（背景込みで「Web に出ている」ことが分かるよう）。
await page.screenshot({ path: join(ROOT, 'hayabusa-web-page.png') });

await browser.close();
server.close();
