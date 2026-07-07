import jsQR from 'jsqr';
import { describe, expect, it } from 'vitest';
import type { LocalNetworkUrl } from './network.js';
import { encodeQr, type QrMatrix } from './qr.js';
import { buildStartupBanner } from './startup-banner.js';

/** banner に埋めた QR と同じテキストを encodeQr した QR を bitmap 化して照合に使う。 */
function decode(qr: QrMatrix): string | null {
  const scale = 4;
  const quiet = 4;
  const dim = (qr.size + quiet * 2) * scale;
  const data = new Uint8ClampedArray(dim * dim * 4);
  for (let py = 0; py < dim; py++) {
    for (let px = 0; px < dim; px++) {
      const mx = Math.floor(px / scale) - quiet;
      const my = Math.floor(py / scale) - quiet;
      const dark =
        mx >= 0 && my >= 0 && mx < qr.size && my < qr.size ? qr.modules[my]![mx]! : false;
      const v = dark ? 0 : 255;
      const o = (py * dim + px) * 4;
      data[o] = v;
      data[o + 1] = v;
      data[o + 2] = v;
      data[o + 3] = 255;
    }
  }
  return jsQR(data, dim, dim)?.data ?? null;
}

const lan: LocalNetworkUrl[] = [
  { address: '192.168.1.23', interfaceName: 'en0', url: 'http://192.168.1.23:5181' },
];

describe('buildStartupBanner', () => {
  it('lists the loopback and LAN URLs', () => {
    const banner = buildStartupBanner({
      port: 5181,
      loopbackUrl: 'http://127.0.0.1:5181',
      networkUrls: lan,
    });
    expect(banner).toContain('http://127.0.0.1:5181');
    expect(banner).toContain('http://192.168.1.23:5181');
    expect(banner).toContain('en0');
  });

  it('embeds a QR that decodes to the primary LAN URL', () => {
    const banner = buildStartupBanner({ port: 5181, networkUrls: lan });
    // banner に出した QR と、その URL を encodeQr した QR は同じはず（同関数・同入力）。
    expect(decode(encodeQr(lan[0]!.url))).toBe(lan[0]!.url);
    // banner は QR 行（半ブロック）を含む。
    expect(banner).toContain('▀');
  });

  it('prefers a private LAN address over other interfaces for the QR', () => {
    const urls: LocalNetworkUrl[] = [
      { address: '100.64.0.1', interfaceName: 'tailscale0', url: 'http://100.64.0.1:5181' },
      { address: '192.168.0.10', interfaceName: 'wlan0', url: 'http://192.168.0.10:5181' },
    ];
    const banner = buildStartupBanner({ port: 5181, networkUrls: urls });
    // private な 192.168.0.10 を QR 化対象として印付けする。
    const markedLine = banner.split('\n').find((l) => l.includes('◀'));
    expect(markedLine).toContain('192.168.0.10');
  });

  it('degrades gracefully when no LAN interface is found', () => {
    const banner = buildStartupBanner({ port: 5181, networkUrls: [] });
    expect(banner).toContain('検出できませんでした');
    expect(banner).not.toContain('▀');
  });
});
