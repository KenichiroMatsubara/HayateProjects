import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';

const readUtf8 = async (url: URL): Promise<string> => String(await readFile(url));

const html = await readUtf8(new URL('../index.html', import.meta.url));
const otherWebDemos = await Promise.all([
  readUtf8(new URL('../../react-demo/index.html', import.meta.url)),
  readUtf8(new URL('../../draw-gallery/index.html', import.meta.url)),
]);

describe('Web renderer switch', () => {
  it('sources backend choices from Hayate Host in every Web demo', () => {
    for (const demoHtml of [html, ...otherWebDemos]) {
      expect(demoHtml).toContain('@torimi/hayate-host/renderer-policy');
      expect(demoHtml).not.toMatch(/data-renderer="(?:vello|tiny-skia)"/);
      expect(demoHtml).toContain('data-renderer="dom"');
    }
  });

  it('renders a single optimization row', () => {
    expect(html.match(/class="rsw-optimize-label"/g)).toHaveLength(1);
    expect(html).not.toContain('id="cpu-layer-present-options"');
  });
});
